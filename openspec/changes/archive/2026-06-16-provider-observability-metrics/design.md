## Decision status

**Status:** ready to implement (decisions locked 2026-06-16)

## Goals

1. One recording hook per upstream attempt so OTEL, REST snapshot, and response headers stay
   consistent.
2. Metrics must survive OTEL export and also be readable synchronously from memory.
3. Per-token latency must be meaningful for streaming and non-streaming calls.
4. Failover semantics must match how multi-hop AI gateways are observed in production OTEL
   stacks: every upstream hop is visible, client outcome stays separate.

## Non-goals

- Persistent historical storage (SQLite, Postgres, object store).
- Billing-grade token accounting when upstream omits usage.
- Replacing Helicone observability logging when enabled.

## Dual-layer observability model

Production AI gateways (OTEL GenAI conventions, Kong AI Gateway, multi-provider routers)
separate two scopes:

| Layer | Scope | Purpose | Existing family |
|-------|--------|---------|-----------------|
| **Upstream attempt** | Each provider/credential HTTP dispatch, including failed failovers | Provider stability, status-code mix, per-provider latency and tokens | New `gateway_provider_*` |
| **Client request** | One inbound API call until the gateway returns to the caller | End-user SLO, terminal usage, response attribution | Existing `router_*` |

Rules:

1. **Every upstream hop** increments attempt-level counters for that `(provider, credential)`.
2. **Only the terminal hop** populates the client JSON header and `router_*` token totals.
3. Failed hops before terminal success increment `status_codes` and `outcome` for the failed
   provider but do **not** add generation-per-token histograms when no output body was
   produced.
4. Each attempt carries `attempt_index` (0-based within the client request) so collectors can
   compute fallback rate: `rate(calls{attempt_index>0}) / rate(calls)`.
5. Token counters on failed hops increment only when usage or partial usage was parsed from
   the error body; otherwise skip tokens for that hop.

This matches the operational pattern where traces show sibling CLIENT spans per provider try
while the SERVER root span reflects what the caller received.

## Metric model (attempt layer)

Shared attributes on every upstream attempt:

| Attribute | Values | Notes |
|-----------|--------|-------|
| `provider` | configured provider id | required |
| `credential` | credential slot id | `default` when single-account |
| `model` | resolved upstream model | `unknown` when absent |
| `router_id` | logical router | `none` for direct proxy |
| `attempt_index` | `0`, `1`, … | position in failover chain |
| `status_code` | HTTP status as integer | upstream response |
| `status_class` | `2xx`…`5xx`, `unknown` | derived |
| `stream` | `true` / `false` | request flag |
| `request_kind` | `router`, `unified_api`, `direct_proxy` | existing enum |
| `outcome` | see quality enum below | derived |
| `usage_source` | `reported`, `estimated`, `none` | on token rows only |

### Token counters (OTEL)

Instrument: `gateway_provider_tokens_total` (Counter, unit: `{token}`)

| `token_type` | Meaning |
|--------------|---------|
| `input` | prompt / input tokens (reported or estimated) |
| `output` | completion / output tokens |
| `cached` | cache-read tokens when reported |
| `reasoning` | reasoning/thinking tokens when reported |
| `total` | upstream total when reported or derived |

When upstream usage is missing on HTTP 2xx, the gateway SHALL run the existing token-estimate
module (default **on** via `observability.estimate_tokens: true`), increment counters with
`usage_source=estimated`, and classify the attempt as `outcome=success_degraded`.

When estimation is disabled and usage is absent, record `success_degraded` without token
increments.

### Call quality

Instrument: `gateway_provider_calls_total` (Counter)

| `outcome` | Rule |
|-----------|------|
| `success` | `200 <= status < 300` and `usage_source=reported` |
| `success_degraded` | `200 <= status < 300` and usage estimated or absent |
| `client_error` | `400 <= status < 500` |
| `server_error` | `500 <= status < 600` |
| `rate_limited` | `status == 429` (also counted in `client_error`) |
| `overload` | `status == 503` with provider overload semantics |

Instrument: `gateway_provider_responses_by_status_total` (Counter) with exact `status_code`.

### Latency

| Instrument | Unit | Value |
|------------|------|-------|
| `gateway_provider_request_duration_ms` | ms | wall time until upstream body complete |
| `gateway_provider_tfft_ms` | ms | time to first token when streaming |
| `gateway_provider_generation_ms_per_output_token` | ms/token | `(duration_ms - tfft_ms) / max(output_tokens, 1)` when `output_tokens >= 1` |

REST JSON also exposes `output_tokens_per_sec` as a derived field for dashboards.

### Existing metrics

Keep emitting `llm_provider_*` and `router_*`. Dual-write during transition. Fix missing
`llm_provider_requests` increment.

## In-memory runtime registry

Keyed by `(provider, credential)`. Attempt-level aggregation since process start.

```json
{
  "started_at": "2026-06-16T12:00:00Z",
  "uptime_seconds": 3600,
  "providers": [
    {
      "provider": "groq",
      "credential": "default",
      "calls": {
        "attempts": 125,
        "success": 110,
        "success_degraded": 3,
        "client_error": 8,
        "server_error": 4
      },
      "status_codes": { "200": 110, "429": 6, "500": 4, "502": 1 },
      "tokens": {
        "input": 450000,
        "output": 120000,
        "cached": 8000,
        "reasoning": 2000,
        "estimated_input": 1200,
        "estimated_output": 300
      },
      "latency": {
        "avg_duration_ms": 840,
        "avg_tfft_ms": 320,
        "avg_generation_ms_per_output_token": 18.5,
        "p50_generation_ms_per_output_token": 16.0,
        "p95_generation_ms_per_output_token": 42.0
      },
      "last_call_at": "2026-06-16T12:59:01Z",
      "last_error_at": "2026-06-16T12:45:00Z",
      "last_status_code": 200
    }
  ],
  "routing": {
    "client_requests": 100,
    "requests_with_failover": 18,
    "failover_rate": 0.18
  }
}
```

`routing.*` fields summarize client-request scope (terminal outcomes only).

## REST API

`GET /v1/observability/provider-stats`

- **Auth:** none — same trust model as `/health` (gateway sits behind private network /
  ingress; not intended for public internet).
- Query: optional `provider`, `credential`, `router_id` filters.
- Response: schema above; counters reset on process restart.
- Errors: `503` if registry not initialized.

Optional `GET /v1/observability/provider-stats/{provider}` for a single entry.

## Response header (single JSON object)

Use **one** header instead of many flat headers. Name: **`X-Gateway-Provider-Usage`**.

Value: compact JSON (UTF-8, no pretty-print). Must stay under 4 KiB; omit null fields.

Terminal hop only (what actually served the client):

```json
{
  "provider": "groq",
  "credential": "default",
  "model": "llama-3.3-70b",
  "usage": {
    "input": 100,
    "output": 40,
    "cached": 0,
    "reasoning": 0,
    "total": 140,
    "source": "reported"
  },
  "latency_ms": {
    "total": 1200,
    "ttft": 250,
    "generation_per_output_token": 23.8
  },
  "routing": {
    "attempts": 2,
    "failover": true
  }
}
```

Field rules:

| Field | Notes |
|-------|-------|
| `usage.source` | `reported` \| `estimated` |
| `usage.*` | integers; estimated values still returned when upstream omitted usage |
| `latency_ms.ttft` | omitted when non-streaming |
| `routing.attempts` | upstream hops for this client request |
| `routing.failover` | `true` when `attempts > 1` |

Config gate: `observability.response_headers.enabled` (default `true`).

Streaming: set header when terminal usage is known (typically last SSE chunk). If usage is
estimated before stream end, header may be set at response start with `"source":"estimated"`
and refined only when the registry allows without buffering the full body.

Do not strip provider-native usage headers; gateway header is additive.

## OTEL export

No change to collector wiring. Document PromQL examples in `docs/observability.md`.

Stability:

```
sum by (provider, status_code) (rate(gateway_provider_responses_by_status_total[5m]))
```

Failover rate:

```
sum(rate(gateway_provider_calls_total{attempt_index!="0"}[5m]))
/
sum(rate(gateway_provider_calls_total[5m]))
```

Generation latency p95:

```
histogram_quantile(0.95,
  sum by (provider, le) (rate(gateway_provider_generation_ms_per_output_token_bucket[5m])))
```

## Coordination with routing-observability

Terminal router trace summary adds `generation_ms_per_output_token`, `upstream_attempts`,
`terminal_outcome`, and `usage_source`.

## Risks

| Risk | Mitigation |
|------|------------|
| JSON header size | Compact JSON, omit nulls, cap at 4 KiB |
| Estimated tokens misleading | `usage.source=estimated` in header + OTEL attribute + trace |
| Double-counting client vs attempt | Document dual-layer model; separate REST `routing` block |
| Public stats endpoint | Document network trust assumption; optional future `observability.public_stats: false` |
