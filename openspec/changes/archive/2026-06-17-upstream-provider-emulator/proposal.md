## Why

External teams and CI must **measure autodefault / budget-aware routing** — failover, pacing,
payload-aware selection, provider-stats, latency histograms — **without live API keys** and
without paying real upstream quotas.

In-process `routing_load` (L1/L2) is fast but not an addressable HTTP service. The legacy
`mock-server` path does not model catalog tiers, per-credential limits, or capabilities.

**The emulated stack is not a toy stub.** It is the **primary way** to obtain routing and
observability numbers for large payloads (e.g. 200 KB+ request bodies, ~35k estimated input
tokens) before production. Live keys are **explicitly out of scope** for this workflow.

A first implementation failed because it (a) returned hardcoded `usage: 6+1` regardless of
payload, (b) did not resolve credential **tier** from `credentials.yaml`, (c) ignored
`model-capabilities` for structured output, (d) used flat latency unrelated to tokens, (e)
left gateway **mapper gaps** (`Converter not present` for catalog providers on failover), and
(f) mixed web-fetch shims with API upstream emulation.

This change specifies a **universal catalog-driven upstream emulator** and the **measurement
contract** external load generators must use to get trustworthy numbers.

## What Changes

- **`upstream-emulator` crate**: one HTTP service reading embedded **`providers.yaml`**,
  **`provider-limits.yaml`**, and **`credentials.yaml` tier names**; dynamic `/{provider_id}/*`
  mounts; catalog-faithful RPM/TPM/RPD/concurrent/min-interval; capabilities-aware responses;
  token-faithful `usage`; configurable latency model.
- **Gateway emulated mode**: automatic rewrite of every API-key `base-url` to the emulator
  (no manual provider URL table in overlay config).
- **Gateway mapper fix**: register `OpenAI → OpenAICompatible` converters for **every** API-key
  `Named` provider in the catalog (failover must not 500 on `longcat`, `bazaarlink`, …).
- **Emulated dev stack**: synthetic secrets, `mise dev:emulated`, smoke + k6 docs.
- **Measurement contract**: autodefault + `openai/gpt-5.4-nano`; assert
  `GET /v1/observability/provider-stats` **and** response `usage` scales with payload size;
  support routing_load-scale fat bodies for payload-aware tests.

## Capabilities

### New Capabilities

- `upstream-provider-emulator`: Universal catalog upstream stub — limits, tiers, capabilities,
  token-faithful usage, latency, protocol-family responses, admin API.
- `emulated-autodefault-dev-stack`: Runnable stack, synthetic secrets, operator/k6 procedure,
  external load acceptance criteria.

### Modified Capabilities

- `provider-observability`: Emulated runs MUST produce stats that reflect estimated tokens and
  emulator latency (not constant 6+1).
- `routing-load-verification`: L3 HTTP verification via emulated stack; fat-payload scenario
  for payload-aware routing.

## Impact

- **New crate:** `crates/upstream-emulator` (dev/CI; not production dependency).
- **Gateway:** `emulated` module (base-url rewrite); **mapper registry** (catalog-driven Named
  providers).
- **Config / dev:** `dev/secrets.emulated.yaml`, `ai-gateway/config/emulated.yaml`,
  `mise dev:emulated`, `dev/emulated-smoke.sh`, `benchmarks/suite/routing-autodefault.js`.
- **Docs:** `DEVELOPMENT.md` — what numbers are trustworthy, what to assert, canonical model.
- **Not affected:** production routing algorithms, release binaries, live credentials.
- **Rejected:** per-provider emulator routes; web `HttpFetch` shim; hardcoded `usage`; manual
  `emulated.yaml` provider URL tables; “use live keys for real numbers” as the load path.
