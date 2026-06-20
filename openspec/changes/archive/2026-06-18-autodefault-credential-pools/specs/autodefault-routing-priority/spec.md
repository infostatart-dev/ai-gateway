## ADDED Requirements

### Requirement: Logical-model binding parity for mini and nano aliases

The gateway SHALL keep `model-mapping.yaml` entries for `gpt-5-mini`,
`gpt-5.4-nano`, and `gpt-5.4-mini` aligned on the **free-tier prefix** through
GitHub and Groq scout targets. For each alias, the ordered free block SHALL
include, when the provider credential resolves:

1. `bazaarlink/auto:free` (when `bazaarlink-default` resolves)
2. `openrouter/openrouter/free` and other live `:free` OpenRouter slugs in YAML order
3. Tier-1 free providers (`bluesminds`, `sambanova`, `ollama-cloud`, OpenCode free models) in YAML order
4. `github-models/openai/gpt-4o-mini`
5. `groq/meta-llama/llama-4-scout-17b-16e-instruct`

Paid fallbacks (`anthropic`, paid `gemini`, …) SHALL remain after the free block.

#### Scenario: gpt-5-mini includes GitHub before Groq scout

- **WHEN** routing `openai/gpt-5-mini` through autodefault
- **AND** `github-models-default` resolves
- **THEN** mapping includes `github-models/openai/gpt-4o-mini` before `groq/meta-llama/llama-4-scout-17b-16e-instruct`
- **AND** both entries precede paid `anthropic` mappings

#### Scenario: gpt-5.4-nano Groq target supports json_schema

- **WHEN** routing `openai/gpt-5.4-nano` with `response_format.type = json_schema`
- **AND** failover reaches the Groq mapping entry
- **THEN** the mapped model is `groq/meta-llama/llama-4-scout-17b-16e-instruct`
- **AND** the candidate passes the Groq json_schema capability filter

#### Scenario: Nano and mini free prefixes stay in sync

- **WHEN** embedded `model-mapping.yaml` is loaded
- **THEN** the first free-tier entries through Groq scout are identical in order
  for `gpt-5-mini`, `gpt-5.4-nano`, and `gpt-5.4-mini`
- **AND** CI mapping-audit test fails if the prefixes diverge

## MODIFIED Requirements

### Requirement: Cost-aligned model binding for default nano model

The gateway SHALL order `model-mapping.yaml` entries for `gpt-5.4-nano` and
`gpt-5.4-mini` so free-tier targets precede paid API targets. The free block
SHALL mirror `gpt-5-mini` per the **Logical-model binding parity** requirement,
including `github-models/openai/gpt-4o-mini` and Groq scout for json_schema.

#### Scenario: Nano mapping prefers free OpenRouter first

- **WHEN** routing `openai/gpt-5.4-nano` through autodefault
- **AND** `openrouter-default` is configured
- **THEN** the first eligible mapping target is a free-tier openrouter entry before any `anthropic` mapping

#### Scenario: Mapping skips unavailable providers

- **WHEN** the first mapping target's provider has no resolved credential
- **THEN** the gateway tries the next mapping entry in YAML order

### Requirement: Canonical autodefault example model

The gateway SHALL use `openai/gpt-5.4-nano` as the documented and CLI-banner
default model for `/router/autodefault/chat/completions`. Operators MAY override
via `AI_GATEWAY_AUTODEFAULT_DEFAULT_MODEL`. Clients that report `openai/gpt-5-mini`
and require live Groq json_schema failover MAY continue doing so until operators
confirm nano mapping parity via the binding-audit test.

#### Scenario: Startup banner shows nano default

- **WHEN** the gateway prints the autodefault curl example
- **THEN** the JSON body uses `"model": "openai/gpt-5.4-nano"`

#### Scenario: Mini remains valid when client reports gpt-5-mini with json_schema

- **WHEN** a client sends `openai/gpt-5-mini` with strict `json_schema`
- **THEN** autodefault selects Groq scout or another json_schema-capable mapped provider
- **AND** GitHub Models is eligible when higher-priority free providers fail
