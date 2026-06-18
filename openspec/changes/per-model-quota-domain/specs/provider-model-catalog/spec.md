## ADDED Requirements

### Requirement: Per-slug limit entries for per-model providers

For providers with `quota-profile: per-model`, `provider-limits.yaml` SHALL declare RPM/TPM/RPD
limits per wire slug (explicit `models:` entries) or per suffix rule where each matching slug
receives an **isolated** pacing gate. The gateway SHALL NOT document or implement a single shared
daily counter across all `:free` slugs on one credential when the upstream API enforces per-slug
quotas.

OpenRouter free tier SHALL declare at minimum:

- `openrouter/free` or `openrouter/openrouter/free`
- `openai/gpt-oss-120b:free`
- `nvidia/nemotron-3-nano-30b-a3b:free`
- `qwen/qwen3-next-80b-a3b-instruct:free`

each with `rpd: 50` (or verified live value) under the free tier.

#### Scenario: Nemotron and gpt-oss resolve distinct limit entries

- **WHEN** `catalog_limit_resolve` runs for `nvidia/nemotron-3-nano-30b-a3b:free` and
  `openai/gpt-oss-120b:free` on OpenRouter free tier
- **THEN** each resolves a `ResolvedModelLimits` with distinct `catalog_model` keys
- **AND** pacing registry creates separate gate keys per slug

---

### Requirement: OpenRouter ListModels verify gate

The repository SHALL provide `catalog:verify-openrouter` asserting every OpenRouter wire slug in
`providers.yaml`, `provider-ladders.yaml`, and per-slug limit entries appears in a frozen
`ai-gateway/tests/fixtures/openrouter-listmodels.json` fixture.

#### Scenario: Phantom OpenRouter slug fails verify

- **WHEN** `provider-ladders.yaml` references a slug absent from the fixture
- **THEN** `catalog:verify-openrouter` exits non-zero

#### Scenario: Verify wired into predeploy when OR YAML changes

- **WHEN** `provider-limits.yaml`, `provider-ladders.yaml`, or OpenRouter fixture changes
- **THEN** `mise run predeploy:rust` depends on `catalog:verify-openrouter`
