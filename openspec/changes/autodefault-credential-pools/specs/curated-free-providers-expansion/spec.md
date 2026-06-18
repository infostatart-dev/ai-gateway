## MODIFIED Requirements

### Requirement: Cost-first model mapping for nano and mini

The gateway SHALL extend `model-mapping.yaml` for `gpt-5.4-nano`, `gpt-5.4-mini`,
and `gpt-5-mini` so free-tier targets precede paid entries. The free block SHALL
include, in order before paid fallbacks:

- `bazaarlink/auto:free`
- `openrouter/openrouter/free`
- `openrouter/nvidia/nemotron-3-nano-30b-a3b:free`
- `bluesminds/gpt-4.1-nano`
- `sambanova/gpt-oss-120b`
- `ollama-cloud/kimi-k2.6`
- live OpenRouter `:free` and OpenCode free models already in embedded config
- `github-models/openai/gpt-4o-mini`
- `groq/meta-llama/llama-4-scout-17b-16e-instruct`

Existing free entries (`openrouter/...:free`, `opencode/...`) SHALL remain and
SHALL stay ahead of paid mappings. **`gpt-5-mini` SHALL include GitHub Models**
in the free block (previously only nano/mini aliases carried GitHub).

#### Scenario: Mini mapping includes GitHub Models

- **WHEN** routing `openai/gpt-5-mini` through autodefault
- **AND** `github-models-default` resolves
- **THEN** `github-models/openai/gpt-4o-mini` appears in the `gpt-5-mini` mapping
- **AND** it precedes `groq/meta-llama/llama-4-scout-17b-16e-instruct`

#### Scenario: Nano mapping prefers free OpenRouter first

- **WHEN** routing `openai/gpt-5.4-nano` through autodefault
- **AND** `openrouter-default` is configured
- **THEN** a free-tier openrouter entry precedes paid `anthropic` mappings

#### Scenario: Mapping skips unavailable providers

- **WHEN** `bazaarlink-default` is not configured
- **THEN** autodefault skips `bazaarlink/auto:free` without error

#### Scenario: Groq scout replaces llama-3.1-8b on nano alias

- **WHEN** embedded `model-mapping.yaml` lists Groq for `gpt-5.4-nano` or `gpt-5.4-mini`
- **THEN** the entry is `groq/meta-llama/llama-4-scout-17b-16e-instruct`
- **AND** `groq/llama-3.1-8b-instant` is not used for those aliases

## ADDED Requirements

### Requirement: Embedded mapping audit for alias parity

The gateway SHALL ship an automated test that compares the free-tier mapping
prefix (from first entry through Groq scout inclusive) across `gpt-5-mini`,
`gpt-5.4-nano`, and `gpt-5.4-mini` and fails CI when the ordered lists diverge.

#### Scenario: CI catches nano-mini drift

- **WHEN** a contributor changes only `gpt-5.4-nano` mappings without updating `gpt-5-mini`
- **THEN** the mapping-audit test fails in CI
