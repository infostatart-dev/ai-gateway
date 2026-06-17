# upstream-provider-emulator

## ADDED Requirements

### Requirement: Universal catalog-driven upstream engine

The upstream emulator SHALL be a single HTTP service that loads the same embedded catalogs the
gateway uses — `providers.yaml`, `provider-limits.yaml`, and credential tier metadata from
`credentials.yaml` — and derives **all** emulation behavior from them: mounts, scope, tier/model
limits, capabilities, token estimates, and latency.

The emulator SHALL NOT ship per-provider Rust route modules. Any API-key provider present in the
embedded catalogs SHALL be mountable without new emulator source files.

#### Scenario: Catalog limit change without code change

- **WHEN** `provider-limits.yaml` changes `rpm` for a provider tier model
- **AND** the emulator process is restarted
- **THEN** enforcement uses the new value
- **AND** no emulator Rust source change was required

#### Scenario: New API-key provider from catalogs only

- **WHEN** a new provider id appears in `providers.yaml` and `provider-limits.yaml` with
  `scope: api-key`
- **THEN** the emulator registers `/{provider_id}/*` on startup
- **AND** serves requests through the resolved protocol family handler

### Requirement: API-key dynamic mounts only

At startup the emulator SHALL iterate the loaded provider catalog and register catch-all routes
under `/{provider_id}/` for every provider with `scope: api-key` in `provider-limits.yaml`.

Browser-session providers SHALL NOT be mounted in the emulated autodefault profile.

#### Scenario: API-key upstream path preserved

- **WHEN** the gateway dispatches to any API-key provider with emulated upstream binding
- **THEN** the HTTP request arrives under `/{provider_id}/` with the same path suffix the
  production `base-url` would have used
- **AND** limits for that `provider_id` are applied from the catalog

### Requirement: Credential tier and model limit resolution

The emulator SHALL resolve limits from `(provider_id, credential_tier, model)` using:

1. Credential tier from the authenticated credential slot (via secrets/registry mapping), not
   the catalog default tier alone.
2. Model slug from the request body (normalized per provider).
3. `provider-limits.yaml` tier → model → `limits` block, with tier-level fallback and suffix
   rules (e.g. `:free`).

The emulator SHALL enforce **RPM**, **TPM** (rolling 60s window), **RPD** (when defined),
**concurrent** in-flight, and **min-interval-ms** from the resolved limits — never from
hardcoded per-provider constants in Rust.

#### Scenario: Credential tier selects RPM bucket

- **WHEN** two synthetic credentials for the same provider map to different tiers in
  `credentials.yaml`
- **THEN** each credential uses the RPM limit of its tier for the resolved model

#### Scenario: Model-specific RPM

- **WHEN** two models under the same provider tier have different `rpm` values in the catalog
- **THEN** exhausting RPM for model A does not exhaust RPM for model B on the same credential

#### Scenario: TPM exhaustion uses token estimate

- **WHEN** cumulative **estimated** input tokens for a credential exceed catalog TPM within the
  rolling window
- **THEN** the emulator returns HTTP 429 with a JSON body parseable by gateway retry-after logic

#### Scenario: Min-interval pacing

- **WHEN** requests for the same scope arrive faster than catalog `min-interval-ms`
- **THEN** the emulator returns HTTP 429 before rendering a success response

### Requirement: Per-credential limit isolation

For API-key providers, limit state SHALL be keyed by `(provider_id, credential_fingerprint)`.

#### Scenario: Independent credentials

- **WHEN** credential A exhausts its RPM bucket for a provider
- **AND** credential B remains under the limit
- **THEN** requests authenticated as B return HTTP 200
- **AND** requests authenticated as A return HTTP 429

### Requirement: Token-faithful usage in responses

The emulator SHALL compute `usage` (or family-equivalent token fields) using the **same token
estimation algorithm** the gateway uses for routing (`token_estimate` module or shared crate),
applied to the incoming request body and the rendered assistant content.

Hardcoded constant token counts (e.g. always `prompt_tokens: 6`) are **forbidden**.

#### Scenario: Fat payload produces large prompt_tokens

- **WHEN** a chat completion request body matches the routing_load fat JSON schema payload
  (~200 KB class)
- **THEN** the success response `usage.prompt_tokens` is greater than 1000
- **AND** the value is consistent with the gateway estimate for the same body

#### Scenario: TPM enforcement matches usage

- **WHEN** a credential approaches catalog TPM for a model
- **THEN** TPM bucket accounting uses the same estimated input tokens as returned in `usage`

### Requirement: Capabilities from providers.yaml

The emulator SHALL consult embedded `providers.yaml` `model-capabilities` (via the same
`supports_json_schema` / `supports_json_object` rules as the gateway) when shaping assistant
content.

#### Scenario: json_schema when supported

- **WHEN** the request sets `response_format.type = json_schema`
- **AND** the resolved model has `supports-json-schema: true` in the catalog
- **THEN** assistant content is minimal valid JSON conforming to the request schema

#### Scenario: json_schema when unsupported

- **WHEN** the request sets `response_format.type = json_schema`
- **AND** the resolved model has `supports-json-schema: false`
- **THEN** assistant content is the plain routing-load token `"ok"`

#### Scenario: json_object when supported

- **WHEN** the request sets `response_format.type = json_object`
- **AND** the model supports json_object in the catalog
- **THEN** assistant content is valid JSON (e.g. `{"ok":true}`)

### Requirement: Protocol-family dispatch

The emulator SHALL map each API-key provider to a **protocol family** via one central module
(not per-provider handlers). Supported families SHALL include at minimum: `openai_compat`,
`gemini_openai_compat`, and `anthropic_messages`.

#### Scenario: OpenAI-compat success body

- **WHEN** an `openai_compat` provider receives a chat completion request
- **THEN** the emulator returns HTTP 200 with valid chat completion JSON including token-faithful
  `usage`
- **AND** plain-text assistant `content` is `"ok"` when structured output is not requested

#### Scenario: Streaming

- **WHEN** the request sets `stream: true` on an openai_compat family provider
- **THEN** the emulator returns an SSE stream terminating with `[DONE]`
- **AND** a final chunk includes usage with token-faithful counts

#### Scenario: Family-specific rate-limit JSON body

- **WHEN** an openai_compat family provider returns HTTP 429
- **THEN** the response body is JSON parseable by the gateway OpenAI-compatible client
- **AND** includes `Retry-After` or family-specific exhausted text for `gemini_openai_compat`

### Requirement: Token-proportional upstream latency

The emulator SHALL delay successful responses by:

```text
base_ms + (prompt_tokens + completion_tokens) * ms_per_token
```

plus optional per-provider `base_ms` override and a global multiplier.

Defaults SHALL be configurable without recompiling (env or config file).

#### Scenario: Fat payload has higher latency than hello

- **WHEN** two requests differ only in body size (hello vs fat payload)
- **AND** the same provider and credential are used
- **THEN** the fat request observes greater emulator delay
- **AND** gateway `gateway_provider_request_duration_ms` reflects the difference

### Requirement: Admin control plane

Admin routes SHALL accept connections from loopback only.

#### Scenario: Reset

- **WHEN** operator calls `POST /_admin/reset`
- **THEN** all per-scope limit counters return to initial state

#### Scenario: Inspect state

- **WHEN** operator calls `GET /_admin/state`
- **THEN** JSON lists per-scope RPM/TPM/RPD usage and estimated token totals

#### Scenario: Injected failure profiles

- **WHEN** operator enables `force-auth-error`, `quota-exhausted`, or `overload` on a scope
- **THEN** subsequent matching requests return 401/403, quota-exhausted, or 503 respectively
  until reset

### Requirement: Anti-patterns explicitly forbidden

The emulator implementation SHALL NOT include: per-provider route source files; hand-coded RPM
tables; hardcoded `usage` token constants; `ai-gateway/stubs/` giant JSON blobs; web fetch or
browser-session wire emulation for the autodefault load profile.

#### Scenario: No provider-specific route module

- **WHEN** a reviewer inspects `crates/upstream-emulator/src/`
- **THEN** there are no `routes/{provider}.rs` files
- **AND** dispatch flows through catalog iteration and family handlers only
