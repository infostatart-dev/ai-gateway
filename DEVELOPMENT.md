## Local development

Maintained by [Infostart IT Lab](https://infostart.ru/lab/about/).

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) 1.91+
- [Docker](https://docs.docker.com/get-docker/) and [Docker Compose](https://docs.docker.com/compose/install/) (optional, for Redis/cache stack)

### Setup

1. **Clone the repository**

   ```bash
   git clone https://github.com/infostatart-dev/ai-gateway.git
   cd ai-gateway
   ```

2. **Environment**

   ```bash
   cp .env.template .env
   ```

   Fill in `AI_GATEWAY_CREDENTIAL_*` variables for the provider slots you want
   to enable. See [`.env.template`](.env.template) for naming conventions and
   [README.md](README.md) for configuration overview.

   Optional: set `integrations.helicone.api-key` in the secrets file only if you
   enable Helicone Cloud observability (`helicone.features` in config). See
   [docs/control-plane.md](docs/control-plane.md) — **no** Helicone service is
   required for `cargo rl`; `infrastructure/compose.yaml` does not run port
   `8585`.

3. **Start supporting services** (optional)

   ```bash
   cd infrastructure && docker compose up -d && cd ..
   ```

   Brings up OTEL collector (`4317`), Redis, Grafana — not Helicone Jawn.

4. **Run the gateway**

   ```bash
   cargo run

   # Or with a dev config file (helicone.features: none):
   cargo rl
   ```

### Testing

```bash
# Send an HTTP request against the running gateway
cargo run -p test

# Unit and integration tests
cargo test --tests --all-features
```

### Routing load verification (`testing` feature)

Concurrent autodefault routing checks without live provider keys. Uses synthetic
secrets, Stubr upstream mocks (L2), and per-credential test hooks (L1).

```bash
# Full routing load suite (~25s; pacing_burst includes a real 12s interval wait)
cargo test -p ai-gateway --test routing_load --features testing -- --test-threads=1
```

**Layout**

| Layer | Location | What it validates |
|-------|----------|-------------------|
| L1 | `ai-gateway/src/routing_load/scenarios/*.rs` | Router + `run_failover_candidates` under concurrent load |
| L2 | `harness_round_robin.rs`, `harness_payload_filter.rs` | HTTP dispatch + `GET /v1/observability/provider-stats` |
| Shared | `routing_load/{payload,assert_stats,responses,router}.rs` | Fat payloads, stats helpers, secrets fixture |

**Adding a scenario**

1. Add `ai-gateway/src/routing_load/scenarios/your_case.rs` and export it from `scenarios/mod.rs`.
2. Register one line in `ai-gateway/tests/routing_load.rs` via the `routing_load_test!` macro.
3. Reuse `RoutingLoadHarness::gemini_free_only(N)` or `gemini_prod_like(N)` for secrets; call `prepare_harness_test()` before Harness cases (clears global mock queues).
4. Assert routing via `attempts_for_credential` / `assert_fairness_band`, not response text.

Payload filter scenarios must exceed groq free TPM (~11.4k effective tokens after margin):
use `GROQ_FILTER_EXTRA_CHARS` from `routing_load::payload` and model `openai/gpt-4o-mini`
(maps to groq `llama-3.1-8b-instant` in the embedded catalog).

### Build

```bash
cargo build          # debug
cargo build --release
```

### Emulated autodefault stack (no live API keys)

Measure routing, failover, and `provider-stats` against a **catalog-faithful
upstream emulator** — not live providers.

```bash
# Terminal 1 — emulator + gateway (ports 5151 / 8080)
mise run dev:emulated

# Terminal 2 — smoke (hello + fat payload + stats)
chmod +x dev/emulated-smoke.sh
./dev/emulated-smoke.sh

# Optional k6
k6 run benchmarks/suite/routing-autodefault.js
```

| Payload class | Purpose | Pass criteria |
|---------------|---------|---------------|
| `hello` | wiring smoke | HTTP 200, stats `attempts >= 1` |
| `fat` | payload-aware / TPM | `usage.prompt_tokens > 1000` (not stub 6+1) |
| `burst` | RPM / failover | 429 when catalog RPM exceeded; multiple stats rows |

Env vars:

- `AI_GATEWAY_EMULATED=1` — rewrite all API-key `base-url` → emulator
- `AI_GATEWAY_EMULATOR_URL` — default `http://127.0.0.1:5151`
- `AI_GATEWAY_SECRETS_FILE` — use `dev/secrets.emulated.yaml` (synthetic keys)
- Model: `openai/gpt-5.4-nano` (same as CLI banner)

**Anti-patterns (v0 failures):** hardcoded `usage: 6+1`; manual provider URL
lists in overlay config; `HttpFetch` web shim; missing mapper for catalog Named
providers (`Converter not present` on failover); plain-text 429 bodies.
