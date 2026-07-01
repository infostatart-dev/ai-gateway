# Client Access

Client access controls inbound gateway users and API keys. It is the service
boundary for user sessions, route scopes, and request/token quotas.

## Registry

Configure the gateway:

```yaml
client-access:
  enabled: true
  file: ./dev/client-access.local.yaml
  reload-interval: 1s
  max-body-bytes: 4MiB
  quota-store:
    type: memory
```

The registry is YAML with `version`, `subjects`, `plans`, and `keys`. Keys must
store only `sha256:<hex>` hashes. Generate a hash from a raw key with:

```bash
printf '%s' 'sk-your-client-key' | shasum -a 256
```

Then store it as:

```yaml
hash: "sha256:<hex>"
```

## Scopes

Supported scopes:

| Scope | Allows |
|-------|--------|
| `unified-api` | `/ai/...` |
| `router:<id>` | `/router/<id>/...` |
| `direct:<provider>` | `/<provider>/...` |
| `*` | Any protected route |

Health and explicitly public observability endpoints remain public.

## Limits

Plans define request and token limits over minute, day, and week windows:

```yaml
limits:
  requests:
    per-minute: 60
    per-day: 1000
    per-week: 5000
  tokens:
    per-minute: 120000
    per-day: 1000000
    per-week: 5000000
```

Minute windows are rolling 60-second windows. Day windows reset on UTC days.
Week windows use ISO weeks starting Monday UTC.

## Reload

The gateway loads the registry at startup. When `client-access.enabled` is true,
missing or invalid initial YAML fails startup closed.

After startup, the file is polled on `reload-interval`. Valid updates atomically
replace the in-memory snapshot. Invalid YAML or a deleted file keeps the last
valid snapshot and emits logs/metrics.

To revoke all keys, write a valid file with `keys: {}`.

## Quota Backend

`memory` is process-local and intended for development, tests, and single
process deployments.

Use `redis` for production or multi-replica deployments:

```yaml
quota-store:
  type: redis
  host-url: redis://redis:6379
  connection-timeout: 1s
```

Redis is authoritative for request counters, token counters, and token
reservations. If Redis is unavailable, protected traffic fails closed with 503.
