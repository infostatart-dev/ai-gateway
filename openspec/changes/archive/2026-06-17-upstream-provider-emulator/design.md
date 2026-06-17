## Context

**Stakeholders:** external load teams (k6, agent stacks), gateway operators, CI.

**Problem:** Need **routing and observability numbers** (provider-stats, token totals, failover,
latency, 429 behavior) on real HTTP autodefault **without live API keys**.

**Single source of truth (shared with gateway):**

| Catalog | Drives |
|---------|--------|
| `providers.yaml` | provider ids, `base-url` suffixes, `model-capabilities` |
| `provider-limits.yaml` | tiers, per-model RPM/TPM/RPD, concurrent, `min-interval-ms`, suffix rules |
| `credentials.yaml` | credential slot → provider + **tier name** (maps into provider-limits) |

The emulator MUST NOT duplicate limit or capability tables in Rust.

**Documented failures (do not repeat):**

| Failure | Symptom | Correct approach |
|---------|---------|------------------|
| Hardcoded `usage: 6+1` | 210 KB body still shows 6 prompt tokens in stats | Use gateway **same** `token_estimate` for TPM + response `usage` |
| Tier ignored | Wrong RPM bucket vs credential slot | Resolve tier: `credentials.yaml` slot tier → `provider-limits` tier name |
| Capabilities ignored | `json_schema` strict → plain `"ok"` → structured validation fails | `supports-json-schema` from yaml + fill minimal valid JSON from request schema |
| Flat 60 ms latency | Histograms useless for throughput modeling | `base_ms + ms_per_token * (prompt + completion)` (config defaults) |
| Mapper gaps | `Converter not present` for `longcat`, … on failover | Register OpenAICompatible for **all** catalog Named API-key providers |
| Plain-text 429 body | `Could not deserialize Value` on error path | OpenAI-compat family: JSON error body + `Retry-After` header |
| Web fetch shim | Wrong boundary (transport not upstream) | API-key upstream only; browser-session out of emulated profile |
| Manual provider URL list | Drift when catalog grows | Automatic `base-url` rewrite for all API-key providers |
| Wrong load model | `gpt-4o-mini` in scripts | Always `openai/gpt-5.4-nano` (CLI banner) |

**Constraints:**

- **No live API keys** in emulated mode — this stack **is** how we get numbers.
- Production dispatch algorithms unchanged except mapper registration + emulated binding.
- External entry: `POST /router/autodefault/chat/completions`.

## Goals / Non-Goals

**Goals:**

- Universal upstream emulator on `:5151` — **every** API-key catalog provider mountable without
  new Rust modules.
- Catalog-faithful **tier → model → limits** with per-credential isolation.
- **Token-faithful** TPM enforcement and response `usage` (aligned with gateway estimate).
- **Capabilities-faithful** structured output when `supports-json-schema` and request asks for
  `json_schema` / `json_object`.
- **Latency model** suitable for histogram assertions.
- Documented **L3 measurement contract** for external load (including fat payloads).
- Gateway **failover-safe** mapper for all Named providers.

**Non-Goals:**

- Real LLM quality or semantic answers (content remains `"ok"` or schema-minimal JSON).
- Web-session providers in emulated autodefault load (`chatgpt-web`, `deepseek-web`).
- Per-provider Rust route files — **rejected**.
- Proving absolute production RPS (emulator models catalog limits, not vendor internals).

## Decisions

### D1: Crate `upstream-emulator`

New workspace crate with **CatalogEngine**. Not an extension of `mock-server`.

### D2: Dynamic mounts (API-key only)

Iterate embedded `ProvidersConfig` + `provider-limits` scope:

- `scope: api-key` → `/{provider_id}/*` catch-all
- `scope: browser-session` → **not mounted** in emulated profile (gateway does not route
  autodefault load there without live sessions)

### D3: Tier resolution

For each request:

1. `credential_fingerprint` from `Authorization: Bearer …` (synthetic key string).
2. Match fingerprint to **credential slot** in loaded secrets / registry
   (`groq-default` → tier `free`, etc.) using the same `credentials.yaml` embedded catalog.
3. `model` from request JSON (normalized slug per provider).
4. Look up `provider-limits.yaml` → `tiers[credential_tier]` → model limits (with tier-level
   fallback and suffix rules like `:free`).

**Rejected:** iterating tiers until first model match without credential tier.

### D4: Limit enforcement

Per scope `(provider_id, credential_fingerprint)`:

- RPM, TPM (60s window), RPD (24h when defined), concurrent, min-interval-ms.
- TPM uses **gateway-equivalent token estimate** on request body (not `chars/4` ad hoc if gateway
  has `token_estimate` module — reuse it).

429/quota/503: **family templates** as **JSON** where gateway OpenAI-compat client parses errors.

### D5: Token-faithful usage (critical for external load)

Response `usage` (and Anthropic `input_tokens`/`output_tokens`) MUST be computed from the same
estimate as TPM:

- `prompt_tokens` = estimated input from request body
- `completion_tokens` = estimated output from assistant content (minimal but non-zero for JSON)
- `total_tokens` = sum

**Rejected:** constants `6` and `1`. External teams use these fields to validate payload-aware
routing; hardcoded values invalidate the run.

### D6: Capabilities from `providers.yaml`

Protocol family from catalog fields (`version` → anthropic, gemini host, else openai_compat) —
not hardcoded provider id enums.

Structured output:

- If request has `response_format: json_schema` **and**
  `model_capability::supports_json_schema(providers, provider, model)` → assistant content is
  **minimal valid JSON** for the request schema (shared `web-structured-output` fill).
- If `json_object` and capable → `{"ok":true}`.
- Else plain `"ok"`.

If request asks for json_schema but model yaml says `supports-json-schema: false` → plain `"ok"`
(simulates unsupported upstream; gateway may failover).

### D7: Latency model

```text
delay_ms = base_ms
         + (prompt_tokens + completion_tokens) * ms_per_token
         + optional per_provider_base_ms
```

Defaults in emulator config (e.g. `base_ms=50`, `ms_per_token=0.02`). Global multiplier for
dev tuning. Applied **before** success response; 429 may skip or use minimal delay (documented).

### D8: Universal gateway upstream binding

When `AI_GATEWAY_EMULATED=1`:

```text
for each provider where scope == api-key:
  base_url = {EMULATOR_URL}/{provider_id}{original_path_suffix}
```

`emulated.yaml` has port/telemetry only — **no** `providers:` URL table.

### D9: Gateway mapper — catalog-driven Named providers

`EndpointConverterRegistry` SHALL register `OpenAI → OpenAICompatible` for every `Named`
provider in `ProvidersConfig` with `scope: api-key`, except providers with dedicated converters
(groq, cloudflare, chatgpt-web, deepseek-web).

**Rejected:** hand-maintained list of ~12 providers while catalog has 30+.

### D10: API-key only — no web fetch

Browser-session providers excluded from emulated secrets and autodefault load profile. No
`HttpFetch` redirect.

### D11: Admin (loopback)

`POST /_admin/reset`, `GET /_admin/state`, `POST /_admin/profile` (auth-error, quota-exhausted,
overload).

### D12: Emulated dev stack

| Artifact | Role |
|----------|------|
| `dev/secrets.emulated.yaml` | Synthetic keys for autodefault API-key slots |
| `ai-gateway/config/emulated.yaml` | `:8080`, telemetry, `helicone.features: none` |
| `mise dev:emulated` | emulator + gateway + env |
| `dev/emulated-smoke.sh` | curl + stats + usage sanity |
| `benchmarks/suite/routing-autodefault.js` | k6 + stats poll |

### D13: Measurement contract (how external teams get numbers)

**Entry:** `POST http://localhost:8080/router/autodefault/chat/completions`

**Model:** `openai/gpt-5.4-nano` (same as `cli/helpers.rs`).

**Payload classes:**

| Class | Purpose | Acceptance |
|-------|---------|------------|
| `hello` | smoke | HTTP 200; stats `attempts >= 1` |
| `fat` | payload-aware / TPM | body per `routing_load::fat_json_schema_body`; response `usage.prompt_tokens` **>> 1000**; stats tokens match order of magnitude |
| `burst` | RPM / failover | N sequential requests; 429 when catalog RPM exceeded; failover rows in stats |

**Primary assertions:**

- `GET /v1/observability/provider-stats` — attempts, success, tokens per `(provider, credential)`
- Response `usage` / `X-Gateway-Provider-Usage` — **not** constant 6+1 on fat payloads
- `gateway_provider_request_duration_ms` — reflects emulator latency model

**Not asserted:** LLM answer quality.

## Architecture

```
 External load (k6, agents)          Emulated stack — NO live keys
 ┌──────────────────┐               ┌─────────────────────────────────┐
 │ fat / burst /    │  HTTP :8080   │ ai-gateway autodefault            │
 │ hello payloads   │ ────────────▶ │ pacing + budget_aware + stats   │
 └──────────────────┘               │ AI_GATEWAY_EMULATED=1             │
                                    │  ├─ auto base-url → emulator      │
                                    │  └─ mapper: all Named providers   │
                                    └───────────────┬─────────────────┘
                                                    │
                                                    ▼
                                    ┌─────────────────────────────────┐
                                    │ upstream-emulator :5151         │
                                    │  providers.yaml  (caps, family) │
                                    │  provider-limits (tiers/quotas) │
                                    │  credentials.yaml (tier map)    │
                                    │  /{provider_id}/*  dispatch     │
                                    │  token_estimate → usage + TPM   │
                                    └─────────────────────────────────┘

 Numbers: provider-stats, usage headers, OTel gateway_provider_*
```

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Token estimate ≠ vendor billing | Document: numbers model **routing** behavior, not invoice accuracy |
| Real min-interval (12s) slows burst tests | Admin reset; optional `pacing-scale` env later |
| Credential tier inference from synthetic key | Map Bearer token → slot id in emulated secrets convention (`emu-{slot}`) or registry lookup |

## Migration Plan

1. Implement `upstream-emulator` per this design (fresh — prior code removed).
2. Gateway: emulated binding + catalog mapper registration.
3. Dev stack + docs with **fat payload acceptance** spelled out.
4. L1/L2 emulator tests; L3 gateway stats test; external k6 procedure.
5. Mark tasks done only when fat-payload `usage` assertion passes in CI.

## Resolved Questions

1. **Live keys for “real” numbers?** **No** — emulated stack is the load path; stats must be
   token-faithful.
2. **Stub 6+1 usage?** **Rejected** — breaks external validation.
3. **Web surfaces in emulator?** **Out of scope** for autodefault emulated profile.
4. **Per-provider Rust routes?** **Rejected**.
5. **Load model:** `openai/gpt-5.4-nano`.
