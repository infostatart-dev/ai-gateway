use r2d2::Pool;
use redis::Client;
use uuid::Uuid;

pub(super) fn reserve_atomic(
    pool: &Pool<Client>,
    key: &str,
    amount: i64,
) -> Result<String, String> {
    let mut conn = pool.get().map_err(|e| format!("pool error: {e}"))?;
    let reservation_id = Uuid::new_v4().to_string();
    let usage_key = format!("usage:{key}");
    let reservation_key = format!("res:{reservation_id}");

    redis::pipe()
        .atomic()
        .incr(&usage_key, amount)
        .hset(&reservation_key, "amount", amount)
        .hset(&reservation_key, "key", key)
        .query::<()>(&mut conn)
        .map_err(|e| format!("redis error: {e}"))?;

    Ok(reservation_id)
}

pub(super) fn commit_delta(
    pool: &Pool<Client>,
    key: &str,
    reservation_id: &str,
    final_amount: i64,
) -> Result<(), String> {
    let mut conn = pool.get().map_err(|e| format!("pool error: {e}"))?;
    let reservation_key = format!("res:{reservation_id}");

    let (stored_key, reserved_amount): (Option<String>, Option<i64>) =
        redis::pipe()
            .hget(&reservation_key, "key")
            .hget(&reservation_key, "amount")
            .query(&mut conn)
            .map_err(|e| format!("redis error: {e}"))?;

    let (stored_key, reserved_amount) = stored_key
        .zip(reserved_amount)
        .ok_or_else(|| "reservation not found".to_string())?;
    if stored_key != key {
        return Err("reservation key mismatch".to_string());
    }

    let usage_key = format!("usage:{key}");
    redis::pipe()
        .atomic()
        .incr(usage_key, final_amount - reserved_amount)
        .del(&reservation_key)
        .query::<()>(&mut conn)
        .map_err(|e| format!("redis error: {e}"))?;

    Ok(())
}
