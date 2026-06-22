# Changelog

All notable changes to this project will be documented in this file.

Maintained by [Infostart IT Lab](https://infostart.ru/lab/about/) since 2026-04.
Fork of [Helicone/ai-gateway](https://github.com/Helicone/ai-gateway).

## [0.5.6] - 2026-06-18

**Replay quota block metadata** — plan-time `QuotaSnapshot` now feeds incident replay:
operators see `blocked_reason`, `next_available_at`, and `quota_excluded` in route trace
JSON without re-hitting provider-stats.

### Changed

- **`ReplayRecord`:** winner score breakdown includes optional `blocked_reason` and
  `next_available_at` when `quota_capacity == 0` at plan time
- **`quota_excluded`:** up to eight pool candidates omitted for zero headroom, with
  plan-time block reason (distinct from circuit-open health exclusions)
- **`QuotaSnapshot`:** stores and exposes `next_available_at` from admission verdict

### Quality

- Removed dead `score()` wrapper and `gate_scope_key()` helper
- CI: `RUSTFLAGS=-D dead_code` build gate on `ai-gateway` lib

## [0.5.5] - 2026-06-20

**Quota admission control** for autodefault: the router now treats upstream limits as a
hierarchical feasibility tree (tier → account → model) and refuses planned hops that
cannot succeed — instead of probing with HTTP or short sleeps. Production failover rates
drop when free-tier pools are saturated; operators get explicit signals when the gateway
violates its own admission contract.

### Features

- **Hierarchical `QuotaAdmission`:** catalog-driven `PacingScope` ladder (L0 tier → L1
  account → L2 model) answers «callable now?» before each planned hop
- **Strict admission:** `headroom_score = 0` when any scope reports `next_wait > 0`; no
  sleep-probe on intermediate hops — infeasible candidates skip without attempt counters
- **Hop-time re-admit:** route planner and failover walk re-evaluate admission before
  every hop; parallel work units spread across distinct feasible credential slots
- **Upstream reconcile:** `apply_upstream_reconcile` on `PacingGate` after classified 429
  responses so local pacing reflects upstream reset windows
- **Repeat-429 guard:** upstream 429 on a hop already marked infeasible increments
  `repeat_429_violations` (provider-stats + `gateway_repeat_429_violations_total` OTel)
  — a gateway contract violation, not another cooldown extension

### Changed

- Failover loop skips infeasible hops with trace `skipped` instead of burning attempt budget
- Quota snapshot and plan scoring use strict zero-wait semantics end-to-end
- Provider-stats routing snapshot exposes `repeat_429_violations`; async enrichment path for
  quota observability fields on credential rows

### Fixed

- Per-session pacing scopes no longer collapse unrelated credentials into a
  shared `missing-session` bucket (four-slot DeepSeek failover).
- Per-model Gemini 503 high-demand keeps ladder walking on the same credential
  (model-scoped exhaustion does not apply slot cooldown).

- Unit: `quota_admission`, strict headroom in `budget_aware_plan` / snapshot paths
- Integration: `routing_load` scenarios — zero-repeat-429, parallel account spread,
  per-session DeepSeek, LongCat TPD, hop re-admit

### Planning (OpenSpec)

- Shipped **`hierarchical-quota-admission`** → living spec `quota-admission-control`
- Archived superseded **`proactive-quota-scheduling`**; synced ops hardening, control-plane
  startup, upstream failure signals, and related capabilities into `openspec/specs/` (27 total)
- Phase 2 follow-up: **`distributed-quota-state`** (Redis shared counters for horizontal gateways)

## [0.5.4] - 2026-06-19

Planning and local-dev fixes after verified Ollama Cloud behavior in dev.

### Planning (OpenSpec)

- **`ollama-prompt-json-per-model-quota`:** Ollama Cloud has no native
  `response_format` / structured outputs API — valid JSON via **prompt-injected
  schema** (`json-schema-delivery: prompt`). Spec covers reflection retry,
  24h model-level JSON-validation cooldown, **`quota-profile: per-model`** (403
  Pro slug must not kill the credential), free catalog trim to **`gpt-oss:120b`**
  and **`gpt-oss:20b`**, and weighted session quota operator notes.

### Fixed

- **Local dev telemetry:** committed `local.yaml` uses `exporter: stdout` so
  `cargo rl` does not log OTLP export errors on `:4317` when docker compose
  otelcol is not running.

## [0.5.3] - 2026-06-19

Fix local and sidecar startup when Helicone control plane is unreachable.

### Fixed

- **HTTP startup gate:** sidecar mode no longer awaits Helicone control-plane
  websocket connect before binding `server.port`. `cargo rl` and autodefault work
  without a service on `:8585`.
- **`local.yaml`:** `helicone.features: none` (removed stale `localhost:8585`
  URLs).

### Documentation

- [docs/control-plane.md](docs/control-plane.md) — Helicone sidecar legacy,
  Infostart control plane roadmap.
- [configuration.md](docs/configuration.md), [DEVELOPMENT.md](DEVELOPMENT.md) —
  clarify docker compose does not include Helicone Jawn.

## [0.5.1] - 2026-06-19

First **0.5** release (non-beta). Autodefault replaces blind failover with
caller-aware, quota-aware **route planning** on free-tier pools, plus ops
hardening for deploy without invoker headers.

### Features

- **Caller request context:** middleware parses `X-Agent-Name`,
  `X-Work-Unit-Id`, and `Helicone-Session-Id`; attaches `CallerRequestContext`
  to router requests
- **Default work-unit ladder:** router routes always resolve a non-empty
  `work_unit_id` (`X-Work-Unit-Id` → `Helicone-Session-Id` → `X-Request-Id` →
  generated UUID); `work_unit_source` in route trace
- **Response echo:** optional `X-Work-Unit-Id` on router responses when source is
  `request-id` or `generated` (default on)
- **Route chain planner:** `plan_route_chain()` builds an ordered hop list (max
  **7** upstream attempts per inbound request, one replan on exhaustion) using
  credential health, pacing headroom, intent floor, and ladder bands
- **Caller-aware spread:** stable hash of `(agent, work_unit, credential)` among
  healthy slots — parallel work units stop colliding on the same Gemini key
- **Work-unit route memory:** in-process sticky binding per
  `(agent_name, work_unit_id)` (30 min TTL, 10k entries); prefer hop 0 on
  repeat; invalidate on failoverable binding failure (e.g. 429)
- **Credential health registry:** rolling 5 min window, circuit-open on sustained
  failure (under 10% success after 5 attempts) or 401; slot/project quota exhaustion
  can open circuit for 15 min
- **Quota snapshot:** pacing `peek` at plan time; zero headroom excludes
  candidates before HTTP; cooldown scoring uses `max(slot/model cooldown,
  pacing wait)`
- **Stability escalation:** fast → capacity → stability on the **same**
  credential before cross-provider; deprioritized OpenRouter models blocked when
  Gemini stability band still has headroom
- **ReplayRecord (D19):** route trace carries `plan_snapshot_ts`, hop-0 score
  breakdown, and top-3 alternatives for post-incident replay
- **Provider-stats:** configured credentials with zero attempts appear as
  `status: idle`; per-credential `routing_health` (`circuit_open`, `open_until`,
  `success_rate`, `planner_excluded`); optional `agent_name` on attempt records

### Changed

- **Failover loop** walks the planned chain instead of re-ranking the full pool
  on every hop
- **Ranking** uses `max(slot, model)` cooldown in budget rank
- Replay/trace score field `q_headroom` renamed to **`quota_capacity`** (serde
  alias `q_headroom` retained for one release)

### Documentation

- [routing.md](docs/routing.md): invoker header contract, work-unit ladder, sticky
  vs spread FAQ, stability order, routing health; [invoker-driver-follow-up.md](docs/invoker-driver-follow-up.md)
  for out-of-repo Graphiti driver work

### Testing

- **routing_load:** 33 scenarios on declarative `UpstreamMockScript` (caller-context,
  memory, quota, request-id spread)
- **Unit / integration:** `caller_context`, `credential_health_registry`,
  `budget_aware_plan`, `budget_aware_memory`, `budget_aware_snapshot`,
  `replay_record`, `provider_observability`, `verify_gemini_catalog`,
  `verify_openrouter_catalog`
- **Harness (D8):** `tests/rl/scenarios/`, `src/tests/routing_harness/`
  (`feature = testing`), shared mocks in `crates/gateway-tests`

### Upgrade notes

- **Recommended headers:** send `X-Agent-Name` and `X-Work-Unit-Id` (or
  `Helicone-Session-Id`) on every autodefault call for spread and route memory;
  anonymous traffic still spreads via `X-Request-Id`
- **Invoker driver** changes are **not** in this repo — see
  `docs/invoker-driver-follow-up.md`
- **Observability:** use `GET /v1/observability/provider-stats` (`routing_health`)
  and route trace fields (`route_memory_hit`, `work_unit_source`, `planned_hops`)

## [0.4.2-beta.5] - 2026-06-18

### Features

- **Upstream credential restriction:** normalized `CredentialRestricted` failure
  signal; DeepSeek temporary account blocks map to HTTP 403 `credential_restricted`
  with per-slot cooldown from `restricted_until` (no empty-response retry loops)
- **DeepSeek multi-slot routing:** failover across up to four session credentials;
  a restricted slot poisons only itself — sibling slots stay eligible until their
  own restriction event
- **Upstream emulator:** `403-credential-restricted` wire profile for routing_load
  and local emulation

### Fixed

- **DeepSeek biz JSON:** completion errors with `biz_code` are parsed before SSE
  collection so restriction events fail fast instead of surfacing as 502 empty responses
- **Structured output:** credential restriction on a retry turn exits immediately
  without further schema retries

### Testing

- **routing_load:** DeepSeek credential restriction failover; three-of-four and
  all-four restricted slot matrix; restricted DeepSeek slots then Gemini stability band
- **deepseek-web:** biz-error parser, turn-level restriction mapping, structured-output
  guard when restriction arrives mid-retry
- **Lib coverage floor:** workspace line coverage stays above 48% (`mise run coverage:gate`)

## [0.4.2-beta.4] - 2026-06-18

### Discovered (live OpenRouter probe)

- **Nemotron `:free` 429** (`free-models-per-day`, `X-RateLimit-Remaining: 0`) exhausts
  that slug only; **gpt-oss `:free` still returns 200** on the same API key
- **Paid slug 402** (`never purchased credits`) was incorrectly retiring the whole
  OpenRouter credential via `ExhaustionScope::Project`

### Features

- **Unified per-model quota profile:** `quota-profile: per-model` drives pacing scope,
  ladder filter, exhaustion classification, and rank for any provider (Gemini unchanged;
  OpenRouter is the first additional consumer)
- **OpenRouter free ladder:** fast → capacity → stability → deprioritized (Nemotron last);
  per-slug `rpd: 50` limits in the embedded catalog
- **Catalog verify:** `catalog:verify-openrouter` checks embedded slugs against a frozen
  ListModels fixture (`mise run catalog:verify-openrouter`)
- **Upstream emulator:** `402-never-purchased` and `429-free-models-per-day` wire profiles
  for routing_load and local emulation

### Fixed

- **402 unpaid route → Model scope** on per-model providers so free `:free` siblings keep
  routing after a paid-slug rejection
- **429 `free-models-per-day` → QuotaExhausted** with model-scoped cooldown from
  `X-RateLimit-Reset` (body is still read when only the reset header is present)
- **Cooldown:** short per-model daily reset windows no longer fall back to the 1h
  quota-exhausted default when `X-RateLimit-Reset` is under half that threshold
- **Rank:** removed alphabetical model tie-break; ladder rank and deprioritized band decide
  order among equal-cost candidates
- **Budget probe:** consult cached probe state before the API-key gate;
  `record_payment_required` runs only on Project-scope 402 (not whole-slot retirement)

### Testing

- **routing_load:** nemotron 429 → gpt-oss 200; paid 402 does not block free siblings
- **intent_acceptance:** fast-thinking matrix — gpt-oss before groq; nemotron 429 failover
  to gpt-oss
- **Lib coverage floor:** workspace line coverage stays above 48% (`mise run coverage:gate`)

## [0.4.2-beta.3] - 2026-06-18

### Discovered (stage live probe)

- **Gemini free slots ~50% 404 / ~50% 429, 0% success** after beta.1: phantom catalog
  slugs (`gemini-3.5-flash-preview`) and retired `gemini-1.5-*` upstream ids
- **ListModels:** `gemini-3.5-flash` exists; `gemini-3.5-flash-preview` and `gemini-1.5-*` do not
- **`gemini-2.5-pro` on free** hits billing/429 — stability band moved to `gemini-2.5-flash-lite`

### Features

- **Provider model catalog:** `upstream_slug` vs `catalog_key` in `providers.yaml`;
  CI verify against frozen Gemini ListModels fixture (`catalog:verify-gemini`)
- **Quota-profile scopes:** per-model 404/unsupported retires `(credential, model)`;
  Gemini 503 high-demand cools slot briefly while continuing intra-slot ladder
- **Ladder-only walk:** Gemini free credentials try only `provider-ladders.yaml`
  models (no cartesian `providers × credentials` dead slugs)

### Changed

- **Gemini slugs:** `gemini-3.5-flash` (GA), `gemini-2.5-flash-lite` on free ladder;
  removed phantom `gemini-3.5-flash-preview` and retired `gemini-1.5-*` upstream ids
- **Free stability band:** `gemini-2.5-flash-lite` instead of billing-gated `gemini-2.5-pro`

### Fixed

- **Stage Gemini 404 storm:** 404 no longer retires whole free slot on per-model profile

## [0.4.2-beta.1] - 2026-06-18

### Features

- **Gemini per-model pacing:** separate RPM/TPM/RPD gates per
  `(credential, model)` using refreshed AI Studio free-tier limits
- **Intra-slot model ladder:** fast → capacity → stability bands on the same
  Gemini credential before inter-slot failover (`provider-ladders.yaml`)
- **Scoped exhaustion:** per-model 429 retires `(credential, model)`; project
  billing cap still retires the whole slot and skips free siblings
- **Shared limit resolution:** `catalog_limit_resolve` shared by gateway pacing
  and upstream-emulator
- **Route trace:** terminal route summary includes `quota_scope`,
  `model_ladder_band`, and `model_ladder_position`

### Changed

- **DeepSeek Web pacing:** pass credential tier into upstream pacing acquire hook
- **mise:** add `coverage:lib` / `coverage:report` tasks (`cargo-llvm-cov`)

## [0.4.1-beta.2] - 2026-06-18

### Features

- **Gemini free pool:** sixteen credential slots (`gemini-free` … `gemini-free-16`)
  for round-robin parallelism in autodefault
- **DeepSeek Web pool:** second session slot `deepseek-web-2` with isolated pacing
  gates and credential round-robin

## [0.4.1-beta.1] - 2026-06-18

### Features

- **Autodefault intent routing:** client `model` is interpreted as a routing
  intent tier (fast-thinking for `gpt-5-nano`/`mini`, deep for plain `gpt-5`)
  instead of a strict `model-mapping.yaml` binding; autodefault sets
  `source-model-selection: intent`
- **Intent pool selection:** preferred-tier band first, asymmetric escalation
  (no downgrade below client floor); plain chat widens fast-thinking pool to
  include non-json upstream
- **Observability:** `X-Routing-Intent-Tier`, `X-Routing-Selection-Phase`
  response headers; route trace fields `routing_intent_tier` and
  `routing_selection_phase`

### Fixed

- **Reasoning misclassification:** `gpt-5-nano`/`mini` no longer trigger deep
  reasoning rank boost via substring match on `gpt-5`

### Changed

- **`model-mapping.yaml` role:** optional for autodefault intent mode; strict
  binding remains default for named routers (`source-model-selection: strict`)
- **`autodefault-credential-pools`:** mapping parity tasks are optional after
  intent pool; Gemini×16 / DeepSeek×2 pool expansion remains recommended

## [0.3.0-beta.22] - 2026-06-17

### Features

- **Routing load verification:** extended in-process `routing_load` harness (10
  concurrent scenarios) covering payload pre-flight, daily quota pacing, pacing
  burst, and failover rotation invariants
- **Catalog quota pacing:** proactive RPM/TPM/RPD/TPD gates per credential scope
  with UTC daily reset from `provider-limits.yaml`
- **OpenRouter budget probe:** runtime `key-info` snapshot; skip paid routes when
  credits are exhausted; refresh on HTTP 402
- **ChatGPT Web observability:** `chatgpt_web_turns` and
  `chatgpt_web_upload_parts` in route summary and provider-stats
- **ChatGPT Web chunking:** 45k-token upload parts (parity with DeepSeek Web) for
  large autodefault payloads

### Changed

- **Autodefault guardrails** (informed by concurrent load harness runs): hard
  payload pre-flight (no best-effort overflow tail); provider priority order aligned
  with routing-priority spec; longcat removed from default model mappings
- **Failover policy:** Gemini HTTP 503 classified as transient — rotate across
  free sibling slots instead of skipping the provider band
- **GitHub Models:** normalize OpenAI-compatible response content arrays before
  deserialize
- **Cooldown policy:** provider `quota-exhausted` overrides for daily-cap providers
  (e.g. cloudflare, cerebras)

## [0.3.0-beta.21] - 2026-06-17

### Features

- **`upstream-emulator` crate:** catalog-faithful OpenAI-compatible upstream for
  local autodefault routing, failover, pacing, and `provider-stats` without live
  API keys
- **Emulated dev stack:** `mise run dev:emulated`, `dev/secrets.emulated.yaml`,
  `AI_GATEWAY_EMULATED` base-url rewrite, k6 `routing-autodefault.js` benchmark
- **Gemini free slots:** `gemini-free-5` … `gemini-free-8` credential slots
- **Mapper registry:** auto-register OpenAI-compatible converters for catalog
  Named API-key providers (fixes failover `Converter not present` for longcat,
  bazaarlink, and peers)

### Changed

- **Autodefault:** exclude `opencode` even when credentials are configured;
  longcat leads free-provider budget rank
- **Routing load:** pacing burst scenario uses `PacingRegistry` instead of ad-hoc
  limits

## [0.3.0-beta.20] - 2026-06-17

### Features

- **Curated free providers (Tier 1):** `longcat`, `doubao`, `ollama-cloud`,
  `inclusionai`, `sambanova`, `bluesminds`, `bazaarlink`, `cohere` as
  OpenAI-compatible providers with credential slots, limits, and autodefault
  placement
- **Groq free reclassification:** `groq-default` uses `tier: free` /
  `cost-class: free`
- **OpenRouter Tier 2:** `openrouter/free` router plus additional live `:free`
  slugs (Nemotron, Liquid, Poolside, and others verified 2026-06-17)
- Autodefault priority extended for new free providers; cost-first mappings for
  `gpt-5.4-nano` and `gpt-5-mini`

## [0.3.0-beta.19] - 2026-06-16

### Features

- **DeepSeek Web structured output:** `json_schema` and `json_object` for
  `deepseek-chat` and `deepseek-reasoner` (validation on assistant `content`
  only; bounded retries)
- **DeepSeek Web context chunking:** 128k input budget, 45k-token upload parts,
  multi-turn session reuse, PoW answer cache (45s TTL)
- **`DeepSeekWebConverter`** registered for autodefault / mapper stack (fixes
  `Converter not present`)
- Shared **`web-structured-output`** crate (ChatGPT Web + gateway gate)
- CLI: `deepseek probe --structured-output`, `deepseek probe --context-limit`
- DeepSeek Web catalog `context-window` raised from 65536 → 128000

## [0.3.0-beta.17] - 2026-06-16

### Features

- **Cost-class-first autodefault routing:** credential slots carry `cost-class`
  (`free` | `paid` | `paid-browser`); candidates sort by cost-class before
  `budget-rank` and provider priority
- Autodefault provider order rebalanced: free API → Gemini free → DeepSeek Web →
  paid API → ChatGPT Web **last**
- `gpt-5.4-nano` and `gpt-5.4-mini` model bindings reordered cost-first (mirror
  `gpt-5-mini` pattern)
- CLI/banner default autodefault model: `openai/gpt-5.4-nano` (override:
  `AI_GATEWAY_AUTODEFAULT_DEFAULT_MODEL`)
- `chatgpt-web-default` slot added to embedded `credentials.yaml`

### Breaking

- Operators who relied on browser-first autodefault (ChatGPT Web primary when
  `CHATGPT_BROWSER_CLI` is set) now get free API paths first; ChatGPT Web is
  last-resort only

## [0.3.0-beta.16] - 2026-06-16

### Features

- Payload-aware autodefault routing: token estimate, TPM/context window filter,
  Gemini quota sibling skip, `json_schema_rank`, route trace headers

## [0.3.0-beta.15] - 2026-06-16

### Features

- GitHub Models provider (PAT via `AI_GATEWAY_CREDENTIAL_GITHUB_MODELS_DEFAULT`)

## [0.3.0-beta.14] - 2026-06-16

### Features

- DeepSeek Web browser-session provider (PoW challenge, SSE completion stream)

## [0.3.0-beta.13] - 2026-06-16

### Features

- ChatGPT Web stabilization: warmup cache, abuse-block 4h cooldown, pacing
  (4 rpm / 12s min interval / 1 concurrent)

## [0.3.0-beta.12] - 2026-06-16

### Features

- Gemini free multi-account: four `gemini-free*` slots with round-robin
  load balancing in budget-aware router

## [0.3.0-beta.11] - 2026-06-15

### Build

- Copy patched dependency sources into the Docker chef cook stage before dependency compilation

## [0.3.0-beta.10] - 2026-06-15

### Build

- Patch anthropic-ai-sdk to use rustls-backed reqwest and eliminate native TLS from the dependency graph

## [0.3.0-beta.9] - 2026-06-15

### Build

- Enable wreq BoringSSL symbol prefixing on Linux for coexistence with transitive native TLS clients
- Route OpenTelemetry OTLP export through gRPC-only features to drop unused HTTP client TLS stacks
- Bust Rust CI dependency cache after the TLS stack realignment

## [0.3.0-beta.8] - 2026-06-15

### Build

- Standardize workspace HTTP clients on rustls to avoid native TLS linker conflicts with wreq/BoringSSL on Linux
- Trim native OpenSSL packages from the Docker build and runtime images
- Align credential and routed-identity unit tests with the current provider/model formats

## [0.3.0-beta.7] - 2026-06-15

### Fixes

- Run rustfmt across ai-gateway and resolve remaining clippy violations
- Refactor failover loop, dispatch, and retry-after helpers for CI lint rules

### CI

- Upgrade GitHub Actions checkout to v5 and opt into Node.js 24 for JavaScript actions

## [0.3.0-beta.6] - 2026-06-15

### Fixes

- Restore native-tls for workspace HTTP clients to fix CI linker failures
- Add OpenSSL to the runtime Docker image
- Format chatgpt-web sources for rustfmt CI
- Resolve chatgpt-web clippy violations and reduce sentinel prepare arity

## [0.3.0-beta.5] - 2026-06-15

### CI

- Replace cargo-dist release workflow with native Linux, macOS, and Windows binary builds
- Publish GitHub Releases on version tags with stripped platform artifacts
- Docker workflow builds and pushes to a configurable private registry on main and tags
- Refresh Dockerfile for Rust 1.91, slimmer runtime image, and explicit service entrypoint

### Fixes

- Switch HTTP and WebSocket clients to rustls for OpenSSL-free container runtime

## [0.3.0-beta.4] - 2026-06-14

### Features

- Embedded `credentials.yaml` with provider, tier, and budget-rank slots per upstream account
- `CredentialRegistry` loading secrets from `AI_GATEWAY_CREDENTIAL_<ID>` with legacy env fallbacks
- Budget-aware router builds candidates per credential; cooldown, rank, and failover track credential id
- `X-RealMode-Model-And-Provider` response header reports `credential-id/model`
- Upstream pacing module: concurrent, RPM, and min-interval gates driven by `provider-limits.yaml`
- Dispatcher acquires pacing permit before upstream dispatch
- `chatgpt-web` session limits and cooldown entries in `provider-limits.yaml`
- Startup banner shows default policy tier, cascade mode, fallback chain, and tier override header
- `.env.template` documents universal credential env variable naming
- Cloudflare credential resolution accepts `AI_GATEWAY_CREDENTIAL_CLOUDFLARE_DEFAULT`

### Tests

- Credential env resolver and registry loading tests
- Credential failover integration test for budget-aware router
- ChatGPT web provider limits catalog test

## [0.3.0-beta.2] - 2026-06-14

### Features

- Configurable router cooldown defaults and per-provider overrides in `provider-limits.yaml`
- Upstream-aware 429 cooldown resolution: headers, JSON retry hints, and error-text reset windows
- 429 classification (rate-limit vs quota-exhausted) with distinct fallback durations
- Dispatcher synthesizes `Retry-After` from upstream error bodies when the header is absent
- Shared OpenAI chat response normalizer for Cloudflare and OpenRouter mappers

### Fixes

- Cloudflare map/object `content` no longer breaks serde on successful responses
- OpenRouter responses without `choices` fail fast in the mapper and trigger failover

## [0.2.0-beta.29] - 2025-07-18

### 🚀 Features

- Configurable generic oai handler (#210)
- Mistral support (#211)
- Prompts (#200)
- [ENG-2217] specify version id to pull prompt (#214)
- [ENG-2231] Prompt Templating (#216)
- [ENG-2237] Support true types of prompt inputs (#217)
- Allow configuring middleware for unified api (#219)
- Dynamic router for cloud gateway (#220)
- Authenticate cloud routers using their own api keys (#225)
- Add model weighted router (#230)
- Send router id and deployment target in helicone metadata (#234)
- Cloud provider keys (#233)
- Use Model latency router (#235)
- Validate router config endpoint (#237)
- Add local cloud config to config folder (#238)

### 🐛 Bug Fixes

- Return 429 errors in OpenAI format (#218)
- Temporarily disable validation (#227)
- Resolve bedrock model id issues (#231)
- Respect `AWS_REGION` (#232)
- Router not found panic (#236)

### 💼 Other

- V0.2.0-beta.28 (#228)

### 🚜 Refactor

- Remove "default" router keyword (#215)
- Prep for new latency router (#226)
- Reorg discover module (#229)

### ⚙️ Miscellaneous Tasks

- InitError if store not properly configured (#221)
- Add dependency caching to ci (#222)
- Use router_hash(cuid) for routers instead of router_id(uuid) (#224)

## [0.2.0-beta.28] - 2025-07-11

### 🚀 Features

- Configurable generic oai handler (#210)
- Mistral support (#211)
- Prompts (#200)
- [ENG-2217] specify version id to pull prompt (#214)
- [ENG-2231] Prompt Templating (#216)
- [ENG-2237] Support true types of prompt inputs (#217)
- Allow configuring middleware for unified api (#219)
- Dynamic router for cloud gateway (#220)

### 🐛 Bug Fixes

- Return 429 errors in OpenAI format (#218)
- Temporarily disable validation

### 🚜 Refactor

- Remove "default" router keyword (#215)
- Prep for new latency router (#226)

### ⚙️ Miscellaneous Tasks

- InitError if store not properly configured (#221)
- Add dependency caching to ci (#222)
- Use router_hash(cuid) for routers instead of router_id(uuid) (#224)

## [0.2.0-beta.27] - 2025-07-08

### 🚀 Features

- Configurable retries (#208)

## [0.2.0-beta.26] - 2025-07-08

### 🚀 Features

- *(examples)* Add example for python tool calls with streaming (#198)

### 🐛 Bug Fixes

- Restore debug logs for stream errs (#199)
- Match anthropic tool_result expected role (#202)

### ⚙️ Miscellaneous Tasks

- General s3 clients (#204)
- Cleanup tower types for unified api, router (#205)
- Add default ./config.yaml path (#206)

## [0.2.0-beta.25] - 2025-07-06

### 🚀 Features

- Generic openai handler (#194)
- Add mistral support (#195)

### 🐛 Bug Fixes

- Remove docs for deprecated style of config (#196)

## [0.2.0-beta.24] - 2025-07-05

### ⚙️ Miscellaneous Tasks

- Rustup update and allow dirty on release workflow (#193)

## [0.2.0-beta.23] - 2025-07-04

### ⚙️ Miscellaneous Tasks

- Use newer ubuntu for rust 1.88 on release workflow (#192)

## [0.2.0-beta.22] - 2025-07-04

### ⚙️ Miscellaneous Tasks

- Rustup update on release workflows (#191)

## [0.2.0-beta.21] - 2025-07-04

### 🚀 Features

- [**breaking**] Rate-limiting with redis (#182)

### ⚙️ Miscellaneous Tasks

- Add named inference provider variant (#190)

## [0.2.0-beta.20] - 2025-07-03

### 🚀 Features

- Backwards compat helicone.features config (#189)

### 🐛 Bug Fixes

- Allow default model mappings in config yaml
- Correctly detect mime type in img mappers

### ⚙️ Miscellaneous Tasks

- Better trace logs
- Load env var for helicone key in examples

## [0.2.0-beta.18] - 2025-07-02

### 🚀 Features

- Add more benchmarks
- Setup db listener
- Redis cache benchmark

### 🐛 Bug Fixes

- Misleading err log when observ. disabled
- Propagate errors from streams
- Resolve EnvFilter clone compilation errors in telemetry
- *(app)* Replace hardcoded sleep with proper server ready signaling
- *(config)* Quote model names with colons in mapping
- *(config)* Isolate model mapping to prevent parsing errors
- *(catch_panic)* Address inefficient to_string call
- *(app)* Resolve type mismatches
- *(server)* Adjust startup sequence and fix telemetry shutdown
- Propagate errors from streams

### 🚜 Refactor

- *(config)* Improve configuration loading logic for Secret types
- Break down oversized main function into focused helpers
- Break down oversized App::new function into focused helper methods
- *(router)* Eliminate code duplication in PathAndQuery extraction
- *(error)* Replace generic Box<dyn Error> with specific error types

### 🎨 Styling

- Apply cargo fmt formatting
- Cargo fmt

### ⚙️ Miscellaneous Tasks

- Bump cargo deps
- Improve mock server config

## [0.2.0-beta.16] - 2025-07-01

### ⚙️ Miscellaneous Tasks

- Bump release

## [0.2.0-beta.15] - 2025-06-30

### 🚀 Features

- Rename with helicone prefix, terraform resources for flyio
- Add Prometheus production configuration and remove outdated Fly.io README
- Deploy all infra needed for load testing
- Enabled creation and destruction of fly resources
- Fly infra also creates the applications via terraform
- Add redis for cache support (#172)

### 🐛 Bug Fixes

- Remove machine creation from terraform resources (fly.toml)
- Removed coloring via peacock extension of settings.json

### 🚜 Refactor

- Remove unused fly machines
- Removed redis from flyio machine

### ⚙️ Miscellaneous Tasks

- Updated to latest gateway spec

## [0.2.0-beta.14] - 2025-06-27

### 🐛 Bug Fixes

- Map error responses to openai errors
- Wrap error response in error key

## [0.2.0-beta.13] - 2025-06-26

### 🚀 Features

- Don't require v1 in path

### 🐛 Bug Fixes

- LLM observability for cached responses
- Streams for mapped providers in unified api

### 📚 Documentation

- Add public beta shield (#163)

### ⚙️ Miscellaneous Tasks

- Fixing tests part 1
- Fixing tests part 2
- Fix test

### ◀️ Revert

- Extend_query

## [0.2.0-beta.11] - 2025-06-26

### 🐛 Bug Fixes

- Extend with query params (#160)

## [0.2.0-beta.10] - 2025-06-26

### 🚀 Features

- Inject v1 if not ther (#158)

### 🚜 Refactor

- Simplify stream chunks

### 📚 Documentation

- *(readme)* Updated content with the latest reviews
- *(readme)* Updated config.yaml based on new releases
- *(readme)* Fixed videos
- *(video)* Improve video

### ⚙️ Miscellaneous Tasks

- Bump to beta.10 (#159)

## [0.2.0-beta.9] - 2025-06-26

### 🚀 Features

- [ENG-2147] Terraform resources for the AI Gateway (#145)
- Render deploy (#144)
- Init benchmark dir (#149)
- Change default ip address in code

### 🐛 Bug Fixes

- Accept-encoding header issue

### 💼 Other

- Add AI gateway server address as a default var

### 📚 Documentation

- Add beta warning on benchmarks (#150)
- Added shield for status: public beta

## [0.2.0-beta.8] - 2025-06-25

### 🚀 Features

- Make it so that users don't need to pass in both ai/v1 just chat/completetions

### 🐛 Bug Fixes

- Use ranges based of num requests to fix test
- Remove export from .env.template
- Fix caching for POST requests
- Max-size kebab casing

### 🚜 Refactor

- Update log levels, messages
- Helicone-observability field -> helicone

### ⚙️ Miscellaneous Tasks

- Bump version (#146)

## [0.2.0-beta.6] - 2025-06-25

### 🚀 Features

- New rust CLI for testing
- Fly IO support (#133)
- Add version tag for --version
- Added pretty welcome messages and reduced log level of many logs

### 🐛 Bug Fixes

- Properly deserialize router names
- Sse streaming prepend with data
- Streaming test

### 📚 Documentation

- Remove helix name from everywhere

### ⚙️ Miscellaneous Tasks

- Add arm64 docker images

## [0.2.0-beta.5] - 2025-06-23

### 🚀 Features

- Add py & TS examples

## [0.2.0-beta.4] - 2025-06-23

### 🚀 Features

- Add warn log when running debug build

## [0.2.0-beta.3] - 2025-06-23

### 🚀 Features

- Health check (#120)
- Add self hosted runners (#122)
- Use redis 8 in ci job

### 🐛 Bug Fixes

- Update grafana dashboard JSON (#115)
- Map Provider name before sending to Jawn (#117)
- Remove protoc step (#121)
- Rename google -> gemini (#118)
- Skip rust ci when unchanged, concurrency limit
- Revert self hosted ci
- Dont crash if jawn is down
- Dont hang in integration test

### 📚 Documentation

- *(readme)* Updated links and snippets
- *(readme)* Fixed naming for ai-gateway and introduced discovery call
- *(readme)* Fixed empty bash command in demo.md

### ⚙️ Miscellaneous Tasks

- Update anthropic-ai-sdk to point upstream (#116)

## [0.2.0-beta.2] - 2025-06-20

### 🐛 Bug Fixes

- Don't err for valid anthropic streams (#114)

## [0.2.0-beta.1] - 2025-06-20

### 🚀 Features

- *(llm-obs)* Llm observ tests
- Add health check monitors for providers
- Replace std HashMap with rustc-hash FxHashMap for performance (#32)
- Add per helicone user rate limit (#34)
- Passthrough reqs for unsupported endpoints
- Add tower-otel-http-metrics
- *(metrics)* Add system level metrics
- *(metrics)* Add provider health metrics
- *(metrics)* Add request/resp count metrics
- *(metrics)* Add better error metrics
- *(metrics)* Add grafana dashboard
- Add viz for error_count and auth metrics
- *(deploy)* Add cargo-chef dockerfile
- Add providers configurations
- Added npm and brew distributions
- Use jemalloc for perf+lower memory usage
- Global and router-level RL configs w/optin
- Add ability to load test
- Use tower-governor for retry-* headers
- Configurable response headers
- Add Ollama provider support
- Setup commit hooks
- Add docker compose
- Rate Limit aware load balancing
- Set auth headers for websockets
- Improved configs + config validation (#72)
- Add github action for docker builds (#67)
- Direct proxy to provider based on URL (#74)
- Better config logging (#75)
- Unified API (#77)
- Track TFFT (#79)
- [ENG-1529] Bedrock Mapper (#60)
- Add LLM observability in sidecar mode (#82)
- Request and response caching (#93)
- Add sidecar yaml, instructions (#95)
- No key required to run gateway, check keys at runtime (#110)
- Better error handling on auth (#108)

### 🐛 Bug Fixes

- Valid model ids are when not mapping providers
- Add enable_control_plane config
- Bedrock headers signing (#92)
- Updated stubr ref (#94)
- Remove `Bearer` when checking api key hash (#96)
- Secret<_> serialize issue when merging configs (#109)
- S3 client issue not being constructed (#112)

### 💼 Other

- Fix path
- V0.2.0-beta.1 (#111)

### 🚜 Refactor

- More robust model id parsing
- Comprehensive embedded model configs
- Use global cp state, remove Mutex
- Better URLs for named routers (#73)
- Remove optin rate limit config (#81)
- Moved websocket mutex to rwlock (#84)
- Update config for launch, remove docs (#101)
- SelfHosted -> Sidecar deployment target (#106)

### ⚙️ Miscellaneous Tasks

- Remove postgres/db stuff
- Add mw to mark sensitive headers
- Code smell
- Clean up
- Basic demo
- Update deps
- Audit error handling
- Remove schema-filter crate (#78)
- Remove duplicate env var (#80)
- Updated npm package name to ai-gateway (#85)
- Update docker name (#86)
- Update toml contributors and links (#87)
- Get releases to work (#99)
- Fix cargo lock pinned rev (#100)
- Fix pre-release typo (#102)
- Remove unused config flag (#107)

<!-- generated by git-cliff -->
