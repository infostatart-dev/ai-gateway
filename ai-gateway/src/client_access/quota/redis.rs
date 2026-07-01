use std::fmt;

use chrono::{DateTime, Utc};
use r2d2::Pool;
use redis::Client;
use uuid::Uuid;

use super::{
    QuotaAdmission, QuotaAdmissionError, QuotaClock, QuotaDimension,
    QuotaFamily, QuotaLimitStatus, QuotaRejection, QuotaReservation,
    QuotaStoreError, QuotaWindowKind, store::ClientAccessQuotaStore,
};
use crate::{
    config::{
        client_access::ClientAccessWindowLimitsConfig, redis::RedisConfig,
    },
    error::init::InitError,
};

pub struct RedisClientAccessQuotaStore {
    pool: Pool<Client>,
    clock: QuotaClock,
}

impl fmt::Debug for RedisClientAccessQuotaStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedisClientAccessQuotaStore")
            .finish_non_exhaustive()
    }
}

impl RedisClientAccessQuotaStore {
    pub fn from_config(config: &RedisConfig) -> Result<Self, InitError> {
        let client = redis::Client::open(config.host_url.expose().clone())
            .map_err(InitError::CreateRedisClient)?;
        let pool = r2d2::Pool::builder()
            .connection_timeout(config.connection_timeout)
            .build(client)
            .map_err(InitError::CreateRedisPool)?;
        Ok(Self::new(pool))
    }

    #[must_use]
    pub fn new(pool: Pool<Client>) -> Self {
        Self {
            pool,
            clock: QuotaClock::default(),
        }
    }
}

#[async_trait::async_trait]
impl ClientAccessQuotaStore for RedisClientAccessQuotaStore {
    async fn admit_request(
        &self,
        key_id: &str,
        limits: &ClientAccessWindowLimitsConfig,
        now: DateTime<Utc>,
    ) -> Result<QuotaAdmission, QuotaAdmissionError> {
        quota_atomic(
            &self.pool,
            &QuotaAtomicInput {
                clock: self.clock,
                key_id,
                family: QuotaFamily::Requests,
                amount: 1,
                limits,
                now,
                reservation_id: None,
            },
        )
    }

    async fn reserve_tokens(
        &self,
        key_id: &str,
        amount: u64,
        limits: &ClientAccessWindowLimitsConfig,
        now: DateTime<Utc>,
    ) -> Result<QuotaReservation, QuotaAdmissionError> {
        let reservation_id = Uuid::new_v4().to_string();
        let admission = quota_atomic(
            &self.pool,
            &QuotaAtomicInput {
                clock: self.clock,
                key_id,
                family: QuotaFamily::Tokens,
                amount,
                limits,
                now,
                reservation_id: Some(&reservation_id),
            },
        )?;
        Ok(QuotaReservation {
            id: reservation_id,
            key_id: key_id.to_string(),
            amount,
            created_at: now,
            admission,
        })
    }

    async fn commit_tokens(
        &self,
        reservation: &QuotaReservation,
        actual_amount: u64,
        now: DateTime<Utc>,
    ) -> Result<(), QuotaStoreError> {
        commit_or_refund(
            &self.pool,
            self.clock,
            reservation,
            actual_amount,
            now,
        )
    }

    async fn refund_tokens(
        &self,
        reservation: &QuotaReservation,
        now: DateTime<Utc>,
    ) -> Result<(), QuotaStoreError> {
        self.commit_tokens(reservation, 0, now).await
    }
}

struct QuotaAtomicInput<'a> {
    clock: QuotaClock,
    key_id: &'a str,
    family: QuotaFamily,
    amount: u64,
    limits: &'a ClientAccessWindowLimitsConfig,
    now: DateTime<Utc>,
    reservation_id: Option<&'a str>,
}

fn quota_atomic(
    pool: &Pool<Client>,
    input: &QuotaAtomicInput<'_>,
) -> Result<QuotaAdmission, QuotaAdmissionError> {
    let mut conn = pool.get().map_err(|err| {
        QuotaStoreError::Operation(format!("pool error: {err}"))
    })?;
    let keys = RedisQuotaKeys::new(
        input.key_id,
        input.family,
        input.clock,
        input.now,
        input.reservation_id,
    );
    let event_id = format!("{}:{}", Uuid::new_v4(), input.amount);
    let script = redis::Script::new(QUOTA_ADMIT_LUA);
    let mut invocation = script.prepare_invoke();
    invocation
        .key(keys.minute)
        .key(keys.day)
        .key(keys.week)
        .key(keys.reservation.unwrap_or_default())
        .arg(input.now.timestamp_millis())
        .arg(event_id)
        .arg(input.key_id)
        .arg(input.amount)
        .arg(input.limits.per_minute.unwrap_or(0))
        .arg(input.limits.per_day.unwrap_or(0))
        .arg(input.limits.per_week.unwrap_or(0))
        .arg(ttl_seconds(
            input.clock.day(input.now).retry_after_seconds(input.now),
        ))
        .arg(ttl_seconds(
            input
                .clock
                .iso_week(input.now)
                .retry_after_seconds(input.now),
        ))
        .arg(input.reservation_id.unwrap_or_default())
        .arg(ttl_seconds(
            input
                .clock
                .iso_week(input.now)
                .retry_after_seconds(input.now)
                .saturating_add(3600),
        ));
    let result: Vec<String> = invocation.invoke(&mut conn).map_err(|err| {
        QuotaStoreError::Operation(format!("redis error: {err}"))
    })?;
    parse_admission_result(input.key_id, input.family, &result)
        .map_err(Into::into)
}

fn commit_or_refund(
    pool: &Pool<Client>,
    clock: QuotaClock,
    reservation: &QuotaReservation,
    actual_amount: u64,
    now: DateTime<Utc>,
) -> Result<(), QuotaStoreError> {
    let mut conn = pool.get().map_err(|err| {
        QuotaStoreError::Operation(format!("pool error: {err}"))
    })?;
    let keys = RedisQuotaKeys::new(
        &reservation.key_id,
        QuotaFamily::Tokens,
        clock,
        now,
        Some(&reservation.id),
    );
    let event_id = Uuid::new_v4().to_string();
    let script = redis::Script::new(QUOTA_COMMIT_LUA);
    let mut invocation = script.prepare_invoke();
    invocation
        .key(keys.minute)
        .key(keys.day)
        .key(keys.week)
        .key(keys.reservation.expect("reservation key is present"))
        .arg(&reservation.key_id)
        .arg(&reservation.id)
        .arg(actual_amount)
        .arg(now.timestamp_millis())
        .arg(event_id)
        .arg(ttl_seconds(clock.day(now).retry_after_seconds(now)))
        .arg(ttl_seconds(clock.iso_week(now).retry_after_seconds(now)));
    let result: Vec<String> = invocation.invoke(&mut conn).map_err(|err| {
        QuotaStoreError::Operation(format!("redis error: {err}"))
    })?;
    match result.first().map(String::as_str) {
        Some("ok") => Ok(()),
        Some("missing") => Err(QuotaStoreError::ReservationNotFound {
            key_id: reservation.key_id.clone(),
            reservation_id: reservation.id.clone(),
        }),
        Some(other) => Err(QuotaStoreError::Operation(other.to_string())),
        None => Err(QuotaStoreError::Operation("empty redis result".into())),
    }
}

struct RedisQuotaKeys {
    minute: String,
    day: String,
    week: String,
    reservation: Option<String>,
}

impl RedisQuotaKeys {
    fn new(
        key_id: &str,
        family: QuotaFamily,
        clock: QuotaClock,
        now: DateTime<Utc>,
        reservation_id: Option<&str>,
    ) -> Self {
        let family = match family {
            QuotaFamily::Requests => "requests",
            QuotaFamily::Tokens => "tokens",
        };
        let day_start = clock.day(now).start.timestamp();
        let week_start = clock.iso_week(now).start.timestamp();
        let prefix = format!("client-access:quota:{{{key_id}}}:{family}");
        Self {
            minute: format!("{prefix}:minute"),
            day: format!("{prefix}:day:{day_start}"),
            week: format!("{prefix}:week:{week_start}"),
            reservation: reservation_id.map(|id| {
                format!("client-access:quota:{{{key_id}}}:reservation:{id}")
            }),
        }
    }
}

fn parse_admission_result(
    key_id: &str,
    family: QuotaFamily,
    result: &[String],
) -> Result<QuotaAdmission, QuotaRejection> {
    if result.first().map(String::as_str) == Some("ok") {
        let Some(window) = parse_window(result.get(1).map(String::as_str))
        else {
            return Ok(QuotaAdmission::default());
        };
        let parse = |idx: usize| -> u64 {
            result
                .get(idx)
                .and_then(|value| value.parse().ok())
                .unwrap_or_default()
        };
        return Ok(QuotaAdmission {
            most_constrained: Some(QuotaLimitStatus {
                dimension: QuotaDimension { family, window },
                limit: parse(2),
                remaining: parse(3),
            }),
        });
    }
    let window = parse_window(result.get(1).map(String::as_str))
        .unwrap_or(QuotaWindowKind::Minute);
    let parse = |idx: usize| -> u64 {
        result
            .get(idx)
            .and_then(|value| value.parse().ok())
            .unwrap_or_default()
    };
    Err(QuotaRejection {
        key_id: key_id.to_string(),
        dimension: QuotaDimension { family, window },
        limit: parse(2),
        used: parse(3),
        requested: parse(4),
        retry_after_seconds: parse(5).max(1),
    })
}

fn parse_window(value: Option<&str>) -> Option<QuotaWindowKind> {
    match value {
        Some("minute") => Some(QuotaWindowKind::Minute),
        Some("day") => Some(QuotaWindowKind::Day),
        Some("week") => Some(QuotaWindowKind::Week),
        _ => None,
    }
}

fn ttl_seconds(seconds: u64) -> usize {
    usize::try_from(seconds.max(1)).unwrap_or(usize::MAX)
}

const QUOTA_ADMIT_LUA: &str = r#"
local minute = KEYS[1]
local day = KEYS[2]
local week = KEYS[3]
local reservation = KEYS[4]
local now_ms = tonumber(ARGV[1])
local event_id = ARGV[2]
local key_id = ARGV[3]
local amount = tonumber(ARGV[4])
local min_limit = tonumber(ARGV[5])
local day_limit = tonumber(ARGV[6])
local week_limit = tonumber(ARGV[7])
local day_ttl = tonumber(ARGV[8])
local week_ttl = tonumber(ARGV[9])
local reservation_id = ARGV[10]
local reservation_ttl = tonumber(ARGV[11])

local function member_amount(member)
  local pos = string.find(member, ":")
  if not pos then return 0 end
  return tonumber(string.sub(member, pos + 1)) or 0
end

redis.call("ZREMRANGEBYSCORE", minute, 0, now_ms - 60000)
local min_used = 0
local minute_members = redis.call("ZRANGE", minute, 0, -1)
for _, member in ipairs(minute_members) do
  min_used = min_used + member_amount(member)
end
local day_used = tonumber(redis.call("GET", day) or "0")
local week_used = tonumber(redis.call("GET", week) or "0")

local function reject(window, limit, used, retry)
  return {"reject", window, tostring(limit), tostring(used), tostring(amount), tostring(retry)}
end

if min_limit > 0 and min_used + amount > min_limit then
  local oldest = redis.call("ZRANGE", minute, 0, 0, "WITHSCORES")
  local retry = 1
  if oldest[2] then
    retry = math.max(1, math.ceil((tonumber(oldest[2]) + 60000 - now_ms) / 1000))
  end
  return reject("minute", min_limit, min_used, retry)
end
if day_limit > 0 and day_used + amount > day_limit then
  return reject("day", day_limit, day_used, day_ttl)
end
if week_limit > 0 and week_used + amount > week_limit then
  return reject("week", week_limit, week_used, week_ttl)
end

local constrained_window = ""
local constrained_limit = 0
local constrained_remaining = 0
local function track(window, limit, used)
  if limit <= 0 then return end
  local remaining = math.max(0, limit - used - amount)
  if constrained_window == "" or remaining < constrained_remaining then
    constrained_window = window
    constrained_limit = limit
    constrained_remaining = remaining
  end
end
track("minute", min_limit, min_used)
track("day", day_limit, day_used)
track("week", week_limit, week_used)

redis.call("ZADD", minute, now_ms, event_id)
redis.call("EXPIRE", minute, 120)
redis.call("INCRBY", day, amount)
redis.call("EXPIRE", day, day_ttl)
redis.call("INCRBY", week, amount)
redis.call("EXPIRE", week, week_ttl)
if reservation_id ~= "" then
  redis.call("HSET", reservation, "key_id", key_id, "amount", amount)
  redis.call("EXPIRE", reservation, reservation_ttl)
end
if constrained_window ~= "" then
  return {"ok", constrained_window, tostring(constrained_limit), tostring(constrained_remaining)}
end
return {"ok"}
"#;

const QUOTA_COMMIT_LUA: &str = r#"
local minute = KEYS[1]
local day = KEYS[2]
local week = KEYS[3]
local reservation = KEYS[4]
local key_id = ARGV[1]
local reservation_id = ARGV[2]
local actual = tonumber(ARGV[3])
local now_ms = tonumber(ARGV[4])
local event_id = ARGV[5]
local day_ttl = tonumber(ARGV[6])
local week_ttl = tonumber(ARGV[7])

local stored_key = redis.call("HGET", reservation, "key_id")
local reserved = tonumber(redis.call("HGET", reservation, "amount") or "")
if stored_key == false or reserved == nil then
  return {"missing"}
end
if stored_key ~= key_id then
  return {"key-mismatch"}
end

local delta = actual - reserved
if delta ~= 0 then
  redis.call("ZREMRANGEBYSCORE", minute, 0, now_ms - 60000)
  redis.call("ZADD", minute, now_ms, event_id .. ":" .. tostring(delta))
  redis.call("EXPIRE", minute, 120)
  local day_after = redis.call("INCRBY", day, delta)
  if day_after < 0 then redis.call("SET", day, 0) end
  redis.call("EXPIRE", day, day_ttl)
  local week_after = redis.call("INCRBY", week, delta)
  if week_after < 0 then redis.call("SET", week, 0) end
  redis.call("EXPIRE", week, week_ttl)
end
redis.call("DEL", reservation)
return {"ok"}
"#;

#[cfg(all(test, feature = "redis-testing"))]
mod tests {
    use std::time::Duration;

    use chrono::TimeZone;

    use super::*;

    const REDIS_URL: &str = "redis://localhost:6340";

    fn redis_store() -> RedisClientAccessQuotaStore {
        let client = redis::Client::open(REDIS_URL).unwrap();
        let pool = r2d2::Pool::builder()
            .connection_timeout(Duration::from_secs(2))
            .build(client)
            .expect("redis-testing requires Redis on localhost:6340");
        RedisClientAccessQuotaStore::new(pool)
    }

    fn unique_key(prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4())
    }

    #[tokio::test]
    async fn redis_client_access_two_stores_share_request_limit() {
        let store_a = redis_store();
        let store_b = redis_store();
        let limits = ClientAccessWindowLimitsConfig {
            per_minute: Some(1),
            per_day: None,
            per_week: None,
        };
        let key = unique_key("shared");
        let now = Utc.with_ymd_and_hms(2026, 6, 29, 12, 0, 0).unwrap();

        store_a.admit_request(&key, &limits, now).await.unwrap();
        assert!(matches!(
            store_b.admit_request(&key, &limits, now).await,
            Err(QuotaAdmissionError::Rejected(_))
        ));
    }

    #[tokio::test]
    async fn redis_client_access_rejected_token_reserve_has_no_partial_increment()
     {
        let store = redis_store();
        let limits = ClientAccessWindowLimitsConfig {
            per_minute: Some(1),
            per_day: Some(1),
            per_week: Some(1),
        };
        let key = unique_key("atomic-token");
        let now = Utc.with_ymd_and_hms(2026, 6, 29, 12, 0, 0).unwrap();

        assert!(matches!(
            store.reserve_tokens(&key, 2, &limits, now).await,
            Err(QuotaAdmissionError::Rejected(_))
        ));
        assert!(store.reserve_tokens(&key, 1, &limits, now).await.is_ok());
    }

    #[tokio::test]
    async fn redis_client_access_sets_window_ttls() {
        let store = redis_store();
        let limits = ClientAccessWindowLimitsConfig {
            per_minute: Some(10),
            per_day: Some(10),
            per_week: Some(10),
        };
        let key = unique_key("ttl");
        let now = Utc.with_ymd_and_hms(2026, 6, 29, 12, 0, 0).unwrap();

        store.admit_request(&key, &limits, now).await.unwrap();

        let mut conn = store.pool.get().unwrap();
        let keys = RedisQuotaKeys::new(
            &key,
            QuotaFamily::Requests,
            store.clock,
            now,
            None,
        );
        let minute_ttl: i64 = redis::cmd("TTL")
            .arg(&keys.minute)
            .query(&mut *conn)
            .unwrap();
        let day_ttl: i64 =
            redis::cmd("TTL").arg(&keys.day).query(&mut *conn).unwrap();
        let week_ttl: i64 =
            redis::cmd("TTL").arg(&keys.week).query(&mut *conn).unwrap();

        assert!(minute_ttl > 0);
        assert!(day_ttl > 0);
        assert!(week_ttl > 0);
    }

    #[tokio::test]
    async fn redis_client_access_refund_after_window_rollover_clamps_to_zero() {
        let store = redis_store();
        let limits = ClientAccessWindowLimitsConfig {
            per_minute: Some(100),
            per_day: Some(100),
            per_week: Some(100),
        };
        let key = unique_key("clamp");
        let monday = Utc.with_ymd_and_hms(2026, 6, 29, 12, 0, 0).unwrap();
        let next_monday = Utc.with_ymd_and_hms(2026, 7, 6, 12, 0, 0).unwrap();

        let reservation = store
            .reserve_tokens(&key, 50, &limits, monday)
            .await
            .unwrap();
        store
            .refund_tokens(&reservation, next_monday)
            .await
            .unwrap();

        let mut conn = store.pool.get().unwrap();
        let keys = RedisQuotaKeys::new(
            &key,
            QuotaFamily::Tokens,
            store.clock,
            next_monday,
            None,
        );
        let day_used: i64 =
            redis::cmd("GET").arg(&keys.day).query(&mut *conn).unwrap();
        let week_used: i64 =
            redis::cmd("GET").arg(&keys.week).query(&mut *conn).unwrap();

        assert_eq!(day_used, 0);
        assert_eq!(week_used, 0);
    }

    #[tokio::test]
    async fn redis_client_access_unavailable_fails_closed() {
        let client = redis::Client::open("redis://127.0.0.1:1").unwrap();
        let pool = r2d2::Pool::builder()
            .connection_timeout(Duration::from_millis(50))
            .build_unchecked(client);
        let store = RedisClientAccessQuotaStore::new(pool);
        let limits = ClientAccessWindowLimitsConfig {
            per_minute: Some(1),
            per_day: None,
            per_week: None,
        };

        assert!(matches!(
            store
                .admit_request("unavailable", &limits, Utc::now())
                .await,
            Err(QuotaAdmissionError::Store(_))
        ));
    }
}
