## 1. Core recording

- [x] 1.1 Add `ProviderAttemptRecorder` in `ai-gateway/src/metrics/provider/` with shared
      struct for tokens, latency, status, outcome, credential
- [x] 1.2 Define OTEL instruments `gateway_provider_*` per `design.md`
- [x] 1.3 Wire recorder into dispatcher metrics logging path (every upstream attempt)
- [x] 1.4 Wire recorder into router failover loop for failed hops before terminal success
- [x] 1.5 Fix `llm_provider_requests` increment (dual-write compatibility)

## 2. Runtime registry + REST

- [x] 2.1 Implement in-memory `ProviderRuntimeStats` with sharded counters + latency reservoir
- [x] 2.2 Add config section `observability` (response headers toggle, estimate flag)
- [x] 2.3 Add `GET /v1/observability/provider-stats` handler (public, like `/health`)
- [x] 2.4 Integration test: two providers, verify JSON totals match recorded attempts

## 3. Response header (single JSON)

- [x] 3.1 Define `GatewayProviderUsage` JSON schema + compact serializer (<4 KiB)
- [x] 3.2 Set `X-Gateway-Provider-Usage` on terminal router response (stream + non-stream)
- [x] 3.3 Set header on direct-proxy terminal responses
- [x] 3.4 Test: JSON parses, `usage.source=estimated` when upstream omits usage, absent when
      config disabled

## 4. Routing trace extension

- [x] 4.1 Extend router trace summary with `generation_ms_per_output_token`,
      `upstream_attempts`, `terminal_outcome`

## 5. Docs and validation

- [x] 5.1 Add `docs/observability.md` (OTEL names, REST schema, header list, PromQL examples)
- [x] 5.2 Run `mise exec -- openspec validate provider-observability-metrics --strict`
- [x] 5.3 Run `cargo test` for new modules + `cargo clippy` on touched files
