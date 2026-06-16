## Why

Operators need comparable provider telemetry in three places at once: OTEL for long-term
dashboards, an in-process REST snapshot since last restart for quick checks, and per-response
headers for client-side attribution. Today the gateway emits partial OTEL counters
(`llm_provider_tokens`, `router_*`) but lacks normalized upstream stability breakdown,
generation latency per output token, a queryable runtime snapshot, and gateway-owned response
headers that mirror provider usage semantics.

## What Changes

- Add a unified **provider observability** surface: OTEL metrics, in-memory runtime registry,
  and response headers driven from the same recording path.
- Emit per-upstream-attempt metrics (every dispatch hop, including failovers) with provider,
  credential, model, HTTP status, token usage, and generation latency per output token.
- Expose unauthenticated `GET /v1/observability/provider-stats` (same trust model as
  `/health`) with attempt-level provider rows plus client-request failover summary.
- Add a single JSON response header `X-Gateway-Provider-Usage` on terminal completions
  (provider, usage with `source`, latency, routing attempts).
- Default token estimation when upstream omits usage; label `usage_source=estimated`.
- Dual-layer metrics: attempt-level `gateway_provider_*` for stability, terminal-only for
  client header and existing `router_*`.
- Wire missing upstream request counters and align naming with existing `llm_*` and
  `router_*` families without breaking current metric names.

## Capabilities

### New Capabilities

- `provider-observability`: OTEL provider throughput/latency/quality metrics, runtime REST
  snapshot since restart, and per-response attribution headers.

### Modified Capabilities

- `routing-observability`: extend terminal router summary to include generation ms/output
  token and upstream attempt counts where not already captured.

## Impact

- `ai-gateway/src/metrics/` (new provider runtime registry + OTEL instruments)
- `ai-gateway/src/dispatcher/service/logging.rs`, router budget-aware completion path
- `ai-gateway/src/endpoints/` (new observability route)
- Response middleware for header injection
- Docs: `docs/configuration.md` or new `docs/observability.md` section for REST + headers
- No database or external storage; counters reset on process restart
