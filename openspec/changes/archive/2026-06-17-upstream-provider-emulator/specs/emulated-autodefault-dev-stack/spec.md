# emulated-autodefault-dev-stack

## ADDED Requirements

### Requirement: Universal automatic upstream binding

When the gateway runs in emulated mode, it SHALL automatically rewrite **every** API-key
provider `base-url` to the local upstream emulator — preserving each provider's original path
suffix from the embedded catalog.

The emulated gateway config overlay SHALL NOT contain a hand-maintained `providers:` URL table.

#### Scenario: All API-key providers point to emulator without manual list

- **WHEN** the gateway starts with `AI_GATEWAY_EMULATED=1`
- **AND** `AI_GATEWAY_EMULATOR_URL` points to `http://127.0.0.1:5151`
- **THEN** each API-key provider's effective `base-url` becomes
  `{EMULATOR_URL}/{provider_id}{original_suffix}`
- **AND** adding a new API-key provider to the embedded catalog requires no emulated.yaml edit

#### Scenario: Browser-session providers excluded from load profile

- **WHEN** the emulated autodefault stack runs with API-key-only synthetic secrets
- **THEN** autodefault load exercises API upstream providers through the catalog emulator
- **AND** the gateway does not install web `HttpFetch` overrides

### Requirement: Catalog-driven mapper for failover

The gateway SHALL register `OpenAI → OpenAICompatible` endpoint converters for every `Named`
API-key provider in the embedded catalog that does not have a dedicated converter, so autodefault
failover does not fail with `Converter not present`.

#### Scenario: Failover to long-tail Named provider

- **WHEN** autodefault selects a catalog provider such as `longcat` or `bazaarlink`
- **THEN** the gateway dispatches without mapper errors
- **AND** the request reaches the emulator under `/{provider_id}/`

### Requirement: Synthetic secrets profile

The repository SHALL ship an emulated secrets file with synthetic **API-key** credentials for
autodefault slots the operator enables — without live provider credentials.

#### Scenario: Autodefault resolves from emulated secrets

- **WHEN** the gateway starts with `AI_GATEWAY_SECRETS_FILE` pointing to the emulated secrets
  file
- **THEN** every credential entry in that file resolves in the credential registry with tier
  names matching `credentials.yaml`
- **AND** autodefault router includes providers that have both catalog entry and credential

#### Scenario: No live keys required

- **WHEN** an operator starts the emulated stack from repository templates only
- **THEN** no environment variable containing a live provider API key is required
- **AND** external load MAY obtain routing numbers solely from this stack

### Requirement: Minimal gateway config overlay

The emulated config overlay SHALL configure only runtime concerns: listen port, telemetry,
helicone features — not per-provider upstream URLs.

#### Scenario: Sidecar autodefault injection

- **WHEN** the gateway loads the emulated overlay on a sidecar deployment target
- **THEN** the `autodefault` router is available at `/router/autodefault`
- **AND** external clients MAY POST chat completions on port 8080 (default)

### Requirement: One-command dev stack

The repository SHALL provide `mise dev:emulated` starting emulator then gateway with emulated
secrets and env flags.

#### Scenario: Mise task

- **WHEN** operator runs the documented mise task
- **THEN** upstream emulator listens on the documented port (default 5151)
- **AND** gateway listens on 8080 with emulated upstream binding enabled

### Requirement: Canonical autodefault load contract

All repository load/smoke tooling for the emulated stack SHALL use model
`openai/gpt-5.4-nano` (overridable via `AI_GATEWAY_AUTODEFAULT_DEFAULT_MODEL`).

#### Scenario: k6 smoke model

- **WHEN** operator runs `benchmarks/suite/routing-autodefault.js`
- **THEN** request bodies use `model: "openai/gpt-5.4-nano"`
- **AND** requests target `POST /router/autodefault/chat/completions`

#### Scenario: Shell smoke validates usage not stubbed

- **WHEN** operator runs `dev/emulated-smoke.sh` with the fat payload variant
- **THEN** it asserts `usage.prompt_tokens` is greater than 1000
- **AND** `provider-stats` shows `attempts >= 1` with token totals consistent with the response

### Requirement: External operator documentation

`DEVELOPMENT.md` SHALL document: ports, env vars, canonical model, **fat vs hello payload
classes**, stats endpoint, usage assertions, and k6 command.

#### Scenario: Operator follows docs for external load

- **WHEN** an operator follows `DEVELOPMENT.md` emulated stack section
- **THEN** they can run load against localhost:8080 autodefault without live keys
- **AND** know which fields prove the run is trustworthy (stats + usage, not assistant text)

### Requirement: Layered verification

Verification SHALL be layered:

1. Emulator crate unit/integration tests (limits, token usage, mounts, HTTP).
2. One minimal gateway integration test: autodefault + canonical model + ephemeral emulator +
   stats and usage assertions on fat payload.
3. External k6 (operator/nightly).

#### Scenario: Gateway E2E fat payload test

- **WHEN** CI runs the emulated wiring integration test
- **THEN** the gateway completes autodefault chat completion with a fat routing_load-scale body
- **AND** response `usage.prompt_tokens` is greater than 1000
- **AND** `GET /v1/observability/provider-stats` shows `calls.attempts >= 1`
