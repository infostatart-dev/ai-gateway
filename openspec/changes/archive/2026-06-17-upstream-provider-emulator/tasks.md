## 1. Catalog engine (`crates/upstream-emulator`)

- [x] 1.1 Add workspace member `crates/upstream-emulator`; binary `:5151`; `bind_ephemeral()` for tests
- [x] 1.2 Load embedded `ProvidersConfig`, `ProviderLimitCatalog`, credential tier metadata
- [x] 1.3 Build `ProviderTable`: id, scope, protocol_family, mount prefix (api-key only)
- [x] 1.4 Central `provider_id → ProtocolFamily` from catalog fields (no per-provider files)
- [x] 1.5 Dynamic axum router: nest `/{provider_id}/*` from catalog iteration
- [x] 1.6 Single dispatch: family → tier/model limits → latency → response

## 2. Limit engine (catalog + credential tier)

- [x] 2.1 Per-scope state keyed by `(provider_id, credential_fingerprint)`
- [x] 2.2 Resolve tier from credential slot / secrets mapping → `provider-limits` tier name
- [x] 2.3 Resolve model from request JSON; apply suffix rules (`:free`, etc.)
- [x] 2.4 Enforce RPM, TPM, RPD, concurrent, min-interval-ms from `QuotaLimits`
- [x] 2.5 TPM accounting uses tiktoken token estimate (same algorithm as gateway)
- [x] 2.6 Family-level 429/quota/503 as **JSON** bodies + `Retry-After` where applicable
- [x] 2.7 Unit tests: tier from credential, per-credential isolation, catalog change → new limits

## 3. Token-faithful usage and capabilities

- [x] 3.1 Response `usage` from token estimate on request body + assistant content
- [x] 3.2 **Reject** hardcoded 6+1 — test: fat body → `prompt_tokens > 1000` (passes)
- [x] 3.3 `supports_json_schema` / `supports_json_object` from `providers.yaml` via gateway helper
- [x] 3.4 `json_schema` → minimal valid JSON via `web-structured-output` fill when supported
- [x] 3.5 Plain `"ok"` when structured output unsupported or not requested
- [x] 3.6 Streaming: final SSE chunk includes token-faithful usage

## 4. Protocol-family responses

- [x] 4.1 `openai_compat`: chat completion JSON, usage, SSE
- [x] 4.2 `gemini_openai_compat`: same wire; distinct exhausted 429 text
- [x] 4.3 `anthropic_messages`: message JSON with token fields
- [x] 4.4 **Do not** use `ai-gateway/stubs/` per-provider JSON blobs

## 5. Latency model

- [x] 5.1 `delay = base_ms + tokens * ms_per_token` (+ per-provider override, multiplier)
- [x] 5.2 Test: fat payload delay > hello payload delay

## 6. Admin control plane (loopback)

- [x] 6.1 `POST /_admin/reset`
- [x] 6.2 `GET /_admin/state` (rpm/tpm/rpd per scope)
- [x] 6.3 `POST /_admin/profile`: force-auth-error, quota-exhausted, overload
- [x] 6.4 Reject non-loopback admin connections

## 7. Gateway emulated binding + mapper

- [x] 7.1 `ai-gateway/src/emulated/`: rewrite all api-key `base-url` when `AI_GATEWAY_EMULATED=1`
- [x] 7.2 Preserve original path suffix in rewritten URL
- [x] 7.3 **Catalog-driven** `OpenAI → OpenAICompatible` for all Named api-key providers
      (fixes `Converter not present` on failover)
- [x] 7.4 Unit tests: rewrite per provider; mapper covers `longcat`, `bazaarlink` etc.

## 8. Emulated dev stack artifacts

- [x] 8.1 `dev/secrets.emulated.yaml` — synthetic keys with explicit tier per slot
- [x] 8.2 `ai-gateway/config/emulated.yaml` — port, telemetry, helicone only (no provider URLs)
- [x] 8.3 `mise dev:emulated` — emulator + gateway + env
- [x] 8.4 `dev/emulated-smoke.sh` — hello + **fat** payload; assert usage and stats
- [x] 8.5 `benchmarks/suite/routing-autodefault.js` — canonical model + stats poll

## 9. Tests (layered)

- [x] 9.1 Emulator HTTP: health, `"ok"`, json_schema fill, 429 JSON isolation
- [x] 9.2 Emulator fat-body test: `usage.prompt_tokens > 1000`
- [x] 9.3 L3 = `dev/emulated-smoke.sh` + k6 (not a gateway harness mock)
- [x] 9.4 No custom router YAML in emulated tests
- [x] 9.5 Gateway retry-after parser accepts emulator 429 JSON — covered: `Retry-After: 1` header + body message `"Please try again in 1s"` both parsed by existing `body.rs` logic

## 10. Documentation

- [ ] 10.1 `DEVELOPMENT.md` — no live keys; hello vs fat payload classes; what numbers mean
- [ ] 10.2 Document v0 failures: 6+1 stub, mapper gaps, plain-text 429, web shim

## 11. Resolved

- [x] 11.1 Live keys for load numbers: **no** — emulated stack is the path
- [x] 11.2 Web surfaces / HttpFetch in emulator: **out of scope**
- [x] 11.3 Per-provider Rust emulators: **rejected**
- [x] 11.4 Load model: **`openai/gpt-5.4-nano`**
- [x] 11.5 Gateway upstream config: **automatic rewrite**
- [x] 11.6 Usage tokens: **token_estimate** — not constants
