## Context

The fork has already recorded that Helicone Cloud sidecar/control-plane is
legacy and not the startup or routing authority. Current request auth is still
implemented through inherited Helicone/control-plane structures, and current
rate limiting is keyed by `AuthContext.user_id` with a request-only governor.
That is the wrong domain for selling or operating this gateway as a service:
the service boundary is the inbound client API key, its scopes, and its quota
policy.

Provider credentials and provider pacing are separate concerns. They protect
upstream accounts and free-tier slots. This change protects the gateway's
inbound service surface and decides which external client key may call which
route and how much traffic it may consume.

## Goals / Non-Goals

**Goals:**

- Make first-party inbound API key access authoritative when configured.
- Keep inbound client keys separate from upstream provider credentials and
  `dev/secrets.local.yaml`.
- Load key metadata, subjects, plans, scopes, and limits from one operator YAML
  file with live reload.
- Keep serving the last valid access snapshot when a later YAML reload is
  invalid.
- Enforce request and token quotas per key over minute, day, and week windows.
- Provide a process-local memory backend for development/single-process
  deployments and Redis as the durable, cross-replica quota authority when
  Redis backend is configured.
- Reserve estimated tokens before dispatch and reconcile against observed usage
  after completion.
- Preserve legacy Helicone auth only when `client-access` is disabled.

**Non-Goals:**

- Build the future Infostart control plane or an admin API.
- Store raw inbound API keys in repository or config files.
- Change upstream provider credential loading from `dev/secrets.local.yaml`.
- Replace provider-side `PacingRegistry` or provider quota admission.
- Make in-memory quota state safe for multi-replica production.

## Decisions

### D1 - New `client-access` config is the entry point

Add main config fields under `client-access`:

```yaml
client-access:
  enabled: true
  file: ./dev/client-access.local.yaml
  reload-interval: 1s
  max-body-bytes: 4MiB
  quota-store:
    type: redis
    host-url: redis://127.0.0.1:6379
```

`enabled: false` preserves today's behavior. When enabled, inbound access is
authoritative and Helicone/control-plane auth is not consulted for protected
routes.

**Rationale:** A separate top-level block avoids overloading `helicone` or
provider `credentials`. It also gives deployment a clear switch for the new
service boundary.

**Alternative rejected:** Reuse `helicone.features: auth`. That couples new
behavior to a documented legacy path and makes the future control plane harder
to reason about.

### D2 - YAML registry stores hashes, subjects, plans, and scopes

The access file shape:

```yaml
version: 1

subjects:
  acme:
    org-id: "00000000-0000-0000-0000-000000000001"
    user-id: "00000000-0000-0000-0000-000000000002"

plans:
  starter:
    max-output-tokens: 4000
    limits:
      requests:
        per-minute: 60
        per-day: 1000
        per-week: 5000
      tokens:
        per-minute: 20000
        per-day: 500000
        per-week: 2000000

keys:
  acme-main:
    hash: "sha256:<hex>"
    subject: acme
    status: active
    plan: starter
    scopes:
      - unified-api
      - router:autodefault
      - direct:openrouter
```

Raw keys are generated outside the runtime and are not accepted in the YAML.
The hash is computed over the Bearer token value after removing the `Bearer `
prefix, using the explicit `sha256:` algorithm marker.

**Rationale:** Hash-only storage keeps the operator file less sensitive and
compatible with external admin tools. Subjects and plans are first-class because
authorization and quota policy need stable ids that do not depend on Helicone
database rows.

**Alternative rejected:** Put inbound keys into `dev/secrets.local.yaml`. That
file is for upstream provider secrets and explicitly rejects policy fields.

### D3 - `ClientAccessContext` is authoritative

Successful inbound auth inserts `ClientAccessContext` into request extensions:

- `key_id`
- `subject_id`
- `user_id`
- `org_id`
- `plan_id`
- `scopes`
- `quota_limits`

For legacy-compatible downstream code, the middleware may also derive the
existing `AuthContext`, but new access and quota decisions must read
`ClientAccessContext`.

**Rationale:** This creates a clean boundary without requiring every downstream
module to be migrated in one patch.

**Risk accepted:** Two auth contexts will temporarily coexist. Tests must prove
that `client-access.enabled` makes `ClientAccessContext` the source of truth.

### D4 - Route scopes are explicit

Supported scopes:

- `unified-api`
- `router:<router-id>`
- `direct:<provider-id>`
- `*` for operator-managed full access

The access layer runs after route classification so it can reject unauthorized
router/direct/unified requests before dispatch. Health endpoints remain
unauthenticated.

**Rationale:** Explicit route scopes are easy to audit in YAML and line up with
the gateway's current route families.

**Alternative rejected:** Scope only by org/user. That cannot restrict a key to
one router or block direct provider proxy calls.

### D5 - Live reload uses polling and last-good snapshots

The runtime starts from a valid initial file. If `client-access.enabled` is true
and the initial file is missing or invalid, startup fails closed.

After startup, a background task polls file metadata on `reload-interval`, reads
the file when it changes, parses and validates the full registry, builds a new
immutable snapshot, then atomically swaps the snapshot used by request handlers.
Invalid reloads keep the last good snapshot and emit logs/metrics.

Deleting the file after startup is treated as an invalid reload. To revoke all
keys, operators must write a valid file with `keys: {}`.

**Rationale:** Polling avoids a new file watching dependency and works with
mounted Kubernetes ConfigMaps/Secrets where native file events are unreliable.
Last-good behavior protects gateway availability from partially written YAML.

**Risk accepted:** A malformed update can leave a revoked key active until a
valid file is written. This is observable and safer than crashing or opening the
gateway.

### D6 - Quota state has memory mode and Redis authority

Two quota backends exist:

- `memory`: process-local, for tests, development, and single-process
  deployments only.
- `redis`: production backend. Redis is the authoritative state for minute,
  day, and week counters plus token reservations.

When `quota-store.type: redis`, quota reserve/check operations use an atomic Lua
script or equivalent transaction that checks all configured request/token
dimensions and creates a reservation in one operation. Redis unavailability
returns a service error for quota-protected traffic instead of silently allowing
unaccounted usage.

**Rationale:** Quotas are service control, not a best-effort cache. Allowing
traffic during Redis outage would make multi-replica limits false and can spend
upstream quota or billable budget incorrectly.

**Alternative rejected:** Implement Redis in a later change. The desired product
semantics are one capability: local YAML policy with quota state that can run
correctly across replicas.

### D7 - Window semantics are explicit

Minute limits use a rolling 60-second window. Day limits use UTC calendar days
with configurable reset hour defaulting to `00:00 UTC`. Week limits use ISO
weeks starting Monday UTC.

The Redis implementation may use sorted sets for rolling minute windows and
TTL-bound bucket keys for day/week windows. The memory implementation must
match observable behavior.

**Rationale:** Rolling minute windows avoid boundary bursts. Calendar day/week
windows match operator expectations for daily and weekly quota grants.

### D8 - Token quota uses reserve, commit, and refund

For protected chat requests, the middleware buffers the request body up to
`max-body-bytes`, estimates input tokens using the existing token estimation
module, reserves output tokens from request `max_tokens` /
`max_completion_tokens`, or the plan `max-output-tokens` default.

Admission reserves `estimated_input + reserved_output` before dispatch. On
response completion:

- If upstream reported usage is available, commit actual total tokens.
- If reported usage exceeds the reservation, commit the overage as quota debt.
- If usage is unavailable on success, commit the reserved estimate.
- If dispatch fails before an upstream attempt, refund the reservation.
- Streaming responses are wrapped so commit/refund happens on body completion
  or error.

**Rationale:** Pre-dispatch admission prevents known over-limit calls. Post
response reconciliation prevents systematic overcharging when estimates are
high, and records debt when usage is higher than the reservation.

**Risk accepted:** A single response can exceed remaining quota after it has
already been served. The system records debt and blocks future traffic; it
cannot unsend the response.

### D9 - Error and header contract

Access failures return OpenAI-shaped errors:

- missing/invalid/suspended/expired key: `401`
- scope denied: `403`
- quota exceeded: `429` with `retry-after`, `x-ratelimit-limit`,
  `x-ratelimit-remaining`, and a dimension label
- Redis/quota backend unavailable in redis mode: `503`

Successful responses include rate-limit headers for the most constrained
request dimension. Token headers are emitted when a token limit was involved in
admission.

**Rationale:** Clients already understand OpenAI-style auth and rate-limit
responses. Headers make admin debugging possible without exposing other keys.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Request body buffering increases memory pressure | Enforce `max-body-bytes`; return a clear client error before dispatch |
| Invalid YAML keeps a key active after intended revocation | Emit reload failure metric/log; require valid `keys: {}` for full revocation |
| Redis outage blocks protected traffic | Intentional fail-closed behavior for production quota correctness |
| Memory backend used with multiple replicas | Document as non-production multi-replica; expose startup warning when enabled |
| Token estimation differs from provider billing | Reconcile with reported usage and carry debt forward |
| Dual `AuthContext` and `ClientAccessContext` create ambiguity | New middleware and tests require client-access decisions to read only `ClientAccessContext` |

## Migration Plan

1. Land the new config and modules with `client-access.enabled: false` by
   default.
2. Add example `dev/client-access.local.example.yaml` and docs showing hash-only
   key setup.
3. Enable memory backend in local/dev smoke tests.
4. Enable Redis backend in production/stage with a mounted access YAML file.
5. Migrate service clients to new gateway keys and keep Helicone auth disabled.
6. Rollback by setting `client-access.enabled: false`; old compatibility auth
   behavior remains available for existing non-service deployments.

## Open Questions

- Exact CLI surface for generating hashed inbound keys can be implemented as
  `ai-gateway client-key generate` or a small documented helper. The runtime
  contract does not depend on the CLI shape.
- Whether weekly reset day should be configurable after v1. This design starts
  with ISO Monday UTC to keep the first implementation deterministic.
