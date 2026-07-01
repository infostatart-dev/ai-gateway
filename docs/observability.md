# Provider observability

The gateway exposes provider usage and quality through three channels that share one
recording path per upstream attempt:

1. **OpenTelemetry** — `gateway_provider_*` instruments for external collectors
2. **REST snapshot** — in-memory totals since process start (public, like `/health`)
3. **Response header** — single JSON object on terminal responses

## Dual-layer model

| Layer | Scope | Metrics |
|-------|--------|---------|
| Upstream attempt | Every provider HTTP dispatch, including failed failovers | `gateway_provider_*` |
| Client request | One inbound call until the gateway responds | `router_*` + REST `routing` block |

Every upstream hop increments attempt-level counters. Only the terminal hop populates
`X-Gateway-Provider-Usage` and client-scoped router totals.

## Configuration

```yaml
observability:
  estimate-tokens: true          # default: estimate when upstream omits usage
  response-headers:
    enabled: true                # default: attach X-Gateway-Provider-Usage
```

When `estimate-tokens` is enabled and upstream usage is missing on HTTP 2xx, counters and
the response header use `usage.source=estimated` and classify the attempt as
`success_degraded`.

## OpenTelemetry instruments

| Instrument | Type | Notes |
|------------|------|-------|
| `gateway_provider_calls_total` | Counter | `outcome` label: success, success_degraded, client_error, … |
| `gateway_provider_responses_by_status_total` | Counter | exact `status_code` |
| `gateway_provider_tokens_total` | Counter | `token_type`, `usage_source` |
| `gateway_provider_request_duration_ms` | Histogram | wall time |
| `gateway_provider_tfft_ms` | Histogram | streaming only |
| `gateway_provider_generation_ms_per_output_token` | Histogram | streaming: `(duration_ms - tfft_ms) / max(output_tokens, 1)`; non-streaming: `duration_ms / max(output_tokens, 1)` |
| `gateway_repeat_429_violations_total` | Counter | 429 on scopes infeasible at hop admit time |

Shared attributes: `provider`, `credential`, `model`, `router_id`, `attempt_index`,
`stream`, `request_kind`, `outcome`.

Existing `llm_provider_*` and `router_*` families are still emitted (dual-write).

### PromQL examples

Status mix by provider:

```promql
sum by (provider, status_code) (
  rate(gateway_provider_responses_by_status_total[5m])
)
```

Failover rate (attempts after the first hop):

```promql
sum(rate(gateway_provider_calls_total{attempt_index!="0"}[5m]))
/
sum(rate(gateway_provider_calls_total[5m]))
```

Generation latency p95:

```promql
histogram_quantile(0.95,
  sum by (provider, le) (
    rate(gateway_provider_generation_ms_per_output_token_bucket[5m])
  )
)
```

## REST snapshot

**Auth:** none — same trust model as `/health` (private network / ingress).

| Route | Description |
|-------|-------------|
| `GET /v1/observability/provider-stats` | All providers since process start |
| `GET /v1/observability/provider-stats/{provider}` | Filter by provider id |

Optional query filters: `credential`, `router_id` (when supported by handler).

Example response shape:

```json
{
  "started_at": "2026-06-16T12:00:00Z",
  "uptime_seconds": 3600,
  "providers": [
    {
      "provider": "openai",
      "credential": "default",
      "calls": {
        "attempts": 125,
        "success": 110,
        "success_degraded": 3,
        "client_error": 8,
        "server_error": 4
      },
      "status_codes": { "200": 110, "429": 6 },
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
        "p95_generation_ms_per_output_token": 42.0
      },
      "last_call_at": "2026-06-16T12:59:01Z",
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

Counters reset on process restart.

## Response header

**Name:** `X-Gateway-Provider-Usage`

**Value:** compact JSON (UTF-8, null fields omitted, max 4 KiB). Terminal hop only.

```json
{
  "provider": "openai",
  "credential": "default",
  "model": "gpt-4o-mini",
  "usage": {
    "input": 100,
    "output": 40,
    "total": 140,
    "source": "reported"
  },
  "latency_ms": {
    "total": 1200,
    "ttfb": 250,
    "ttft": 250,
    "generation_per_output_token": 23.8
  },
  "routing": {
    "attempts": 2,
    "failover": true
  }
}
```

| Field | Notes |
|-------|-------|
| `usage.source` | `reported` or `estimated` |
| `latency_ms.ttfb` | time to first upstream response body byte |
| `latency_ms.ttft` | time to first generated token; omitted for non-streaming |
| `routing.failover` | `true` when `attempts > 1` |

Omit the header entirely when `observability.response_headers.enabled=false`.

Provider-native usage headers are not stripped; this header is additive.

## Routing trace

Terminal budget-aware route summaries include `generation_ms_per_output_token`,
`upstream_attempts`, `terminal_outcome`, and `usage_source` in structured logs.
