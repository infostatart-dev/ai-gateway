# curated-free-providers-expansion

## Purpose

Expand autodefault free inference by adding eight OpenAI-compatible API-key
providers (Tier 1), reclassifying Groq as free, and extending the OpenRouter
free catalog (Tier 2). All providers share credential gating, conservative
capability metadata, provider limits, autodefault placement, and mock-backed
tests.

## Requirements

### Requirement: Shared OpenAI-compatible provider pattern

The gateway SHALL integrate each Tier 1 provider as `InferenceProvider::Named`
using the existing OpenAI-compatible dispatcher. Each provider SHALL define
`base-url`, a curated `models` list, per-model `model-capabilities`, a
`{provider}-default` credential slot with `tier: free` and `cost-class: free`,
and a `free` tier in `provider-limits.yaml`. Missing credentials SHALL omit the
slot without startup failure.

#### Scenario: Named provider dispatches with bearer auth
- **WHEN** a chat completion targets `longcat/LongCat-Flash-Lite` and
  `longcat-default` resolves
- **THEN** the gateway sends an OpenAI-compatible request to the provider
  `base-url`
- **AND** includes `Authorization: Bearer <credential>`

#### Scenario: Unconfigured provider is skipped
- **WHEN** `AI_GATEWAY_CREDENTIAL_SAMBANOVA_DEFAULT` is unset
- **THEN** slot `sambanova-default` is absent from the credential registry
- **AND** startup completes without error

### Requirement: LongCat provider catalog

The gateway SHALL expose `longcat` with base URL `https://api.longcat.chat/openai/`
and SHALL register these chat models with conservative capabilities:

| Model ID | Context window | Tools | JSON schema | Reasoning |
| --- | ---: | :---: | :---: | :---: |
| `LongCat-Flash-Lite` | 131072 | true | true | false |
| `LongCat-Flash-Chat` | 131072 | true | true | false |
| `LongCat-Flash-Thinking` | 131072 | true | false | true |
| `LongCat-Flash-Omni-2603` | 131072 | true | false | false |

`LongCat-2.0-Preview` SHALL be added when live on the upstream platform docs at
implementation time.

#### Scenario: LongCat model routing
- **WHEN** a request uses `longcat/LongCat-Flash-Lite`
- **THEN** upstream model id is `LongCat-Flash-Lite`
- **AND** the request targets `https://api.longcat.chat/openai/v1/chat/completions`

#### Scenario: LongCat credential slot
- **WHEN** `AI_GATEWAY_CREDENTIAL_LONGCAT_DEFAULT` is set
- **THEN** slot `longcat-default` maps to provider `longcat` with `tier: free`

### Requirement: Doubao (Volcengine Ark) provider catalog

The gateway SHALL expose `doubao` with base URL
`https://ark.cn-beijing.volces.com/api/v3/` and SHALL register at minimum
`doubao-pro-32k` with `supports-tools: true`, `supports-json-schema: true`,
`context-window: 32768`.

#### Scenario: Doubao routing
- **WHEN** a request uses `doubao/doubao-pro-32k`
- **THEN** upstream model id is `doubao-pro-32k`

#### Scenario: Doubao credential slot
- **WHEN** `AI_GATEWAY_CREDENTIAL_DOUBAO_DEFAULT` is set
- **THEN** slot `doubao-default` maps to provider `doubao` with `tier: free`

### Requirement: Ollama Cloud provider catalog

The gateway SHALL expose `ollama-cloud` as a distinct provider from local `ollama`
with base URL `https://ollama.com/v1/` and SHALL register at minimum:

| Model ID | Context window | Tools | JSON schema | Reasoning |
| --- | ---: | :---: | :---: | :---: |
| `deepseek-v4-pro` | 200000 | true | false | true |
| `deepseek-v4-flash` | 200000 | true | false | true |
| `kimi-k2.6` | 262144 | true | true | false |
| `kimi-k2.7-code` | 262144 | true | true | false |

#### Scenario: Ollama Cloud is not local Ollama
- **WHEN** operators configure `ollama-cloud-default`
- **THEN** requests do not target `http://localhost:11434/`
- **AND** provider id remains `ollama-cloud`, not `ollama`

#### Scenario: Ollama Cloud credential slot
- **WHEN** `AI_GATEWAY_CREDENTIAL_OLLAMA_CLOUD_DEFAULT` is set
- **THEN** slot `ollama-cloud-default` maps to provider `ollama-cloud`

### Requirement: InclusionAI provider catalog

The gateway SHALL expose `inclusionai` with base URL
`https://api.inclusionai.tech/v1/` and model `inclusion-model` with
`context-window: 131072`, `supports-tools: true`, `supports-json-schema: false`.

#### Scenario: InclusionAI routing
- **WHEN** a request uses `inclusionai/inclusion-model`
- **THEN** upstream model id is `inclusion-model`

### Requirement: SambaNova provider catalog

The gateway SHALL expose `sambanova` with base URL `https://api.sambanova.ai/v1/`
and SHALL register:

| Model ID | Context window | Tools | JSON schema | Reasoning |
| --- | ---: | :---: | :---: | :---: |
| `MiniMax-M2.7` | 131072 | true | true | false |
| `DeepSeek-V3.2` | 131072 | true | true | false |
| `Llama-4-Maverick-17B-128E-Instruct` | 131072 | true | true | false |
| `Meta-Llama-3.3-70B-Instruct` | 131072 | true | true | false |
| `gpt-oss-120b` | 131072 | true | true | true |

#### Scenario: SambaNova structured output candidate
- **WHEN** autodefault routes a JSON-schema request
- **AND** `sambanova-default` resolves
- **THEN** `sambanova/gpt-oss-120b` is an eligible candidate when capability
  filters pass

### Requirement: BluesMinds provider catalog

The gateway SHALL expose `bluesminds` with base URL `https://api.bluesminds.com/v1/`
and SHALL register a curated free-tier subset:

`gpt-4o-mini`, `gpt-4.1-nano`, `gemini-2.0-flash`, `deepseek-reasoner`,
`llama-3.3-70b-instruct`, `qwen3-32b`

Each entry SHALL set `context-window: 128000`. JSON-schema support SHALL be
`true` only for models verified at implementation time.

#### Scenario: BluesMinds aggregator routing
- **WHEN** a request uses `bluesminds/gpt-4.1-nano`
- **THEN** upstream model id is `gpt-4.1-nano` without gateway prefix stripping
  beyond `bluesminds/`

### Requirement: BazaarLink provider catalog

The gateway SHALL expose `bazaarlink` with base URL `https://bazaarlink.ai/api/v1/`
and SHALL register at minimum:

`auto:free`, `gpt-5.4-nano`, `gemini-3-flash-preview`, `deepseek-v3.2`,
`kimi-k2.6`, `llama-3.3-70b-instruct`, `qwen3.6-plus`

`auto:free` SHALL be the preferred BazaarLink target in autodefault mappings.

#### Scenario: BazaarLink auto free routing
- **WHEN** a request uses `bazaarlink/auto:free`
- **THEN** upstream model id is `auto:free`

### Requirement: Cohere compatibility provider catalog

The gateway SHALL expose `cohere` using the OpenAI compatibility layer at
`https://api.cohere.com/compatibility/v1/` and SHALL register:

| Model ID | Context window | Tools | JSON schema |
| --- | ---: | :---: | :---: |
| `command-a-03-2025` | 128000 | true | true |
| `command-a-reasoning-08-2025` | 128000 | true | false |
| `command-r7b-12-2024` | 128000 | true | true |

#### Scenario: Cohere uses compatibility endpoint
- **WHEN** a request targets `cohere/command-a-03-2025`
- **THEN** upstream URL is under `api.cohere.com/compatibility/v1/`
- **AND** response parsing uses the OpenAI-compatible shape

### Requirement: Groq free-tier reclassification

The gateway SHALL change credential slot `groq-default` to `tier: free` and
`cost-class: free`. `provider-limits.yaml` SHALL expose a `groq.tiers.free` tier
with limits equivalent to the current developer free tier. Autodefault SHALL treat
Groq as a free API provider.

#### Scenario: Groq cost-class is free
- **WHEN** `groq-default` resolves at startup
- **THEN** its cost-class is `free`
- **AND** it ranks before paid providers in budget-aware selection

#### Scenario: Groq limits tier key
- **WHEN** embedded provider limits load for Groq
- **THEN** a `free` tier exists with per-model RPM/RPD/TPM limits
- **AND** notes reference Groq console free developer tier

### Requirement: OpenRouter Tier 2 free catalog expansion

The gateway SHALL extend the embedded `openrouter` model list with:

1. Router model `openrouter/free` (upstream id `openrouter/free`).
2. Additional verified `:free` chat slugs, including at minimum:
   - `arcee-ai/trinity-large-preview:free`
   - `arcee-ai/trinity-mini:free`
   - `deepseek/deepseek-r1:free`
   - `nvidia/nemotron-3-nano-30b-a3b:free` (when live in catalog)
3. Any other zero-cost chat-completion slugs returned by
   `https://openrouter.ai/api/v1/models` at implementation time.

Each new slug SHALL include `model-capabilities` consistent with OpenRouter docs
or live probe. Slugs missing from the live catalog at implementation time SHALL
be omitted rather than breaking startup.

#### Scenario: OpenRouter free router model
- **WHEN** a request uses `openrouter/openrouter/free`
- **THEN** upstream model id is `openrouter/free`

#### Scenario: Trinity free slug routing
- **WHEN** a request uses `openrouter/arcee-ai/trinity-large-preview:free`
- **THEN** upstream model id preserves the full slug including `:free`

#### Scenario: Live catalog verification
- **WHEN** implementers refresh the OpenRouter free list
- **THEN** each retained `:free` slug exists in the live models API response
- **AND** removed dead slugs do not remain in `providers.yaml`

### Requirement: Extended autodefault provider priority

The gateway SHALL build autodefault with the following provider priority when
credentials or session files are available (earlier = higher priority within the
same cost-class band):

1. `opencode`
2. `longcat`
3. `mistral`
4. `openrouter`
5. `github-models`
6. `bazaarlink`
7. `bluesminds`
8. `groq`
9. `cerebras`
10. `cloudflare`
11. `sambanova`
12. `inclusionai`
13. `ollama-cloud`
14. `cohere`
15. `doubao`
16. `gemini`
17. `deepseek-web`
18. `anthropic`
19. `openai`
20. `chatgpt-web`

Providers without resolved credentials SHALL be omitted; remaining ranks SHALL
compress without gaps affecting relative order.

#### Scenario: LongCat precedes OpenRouter when configured
- **WHEN** both `longcat-default` and `openrouter-default` resolve
- **THEN** `longcat` has lower provider priority number than `openrouter`

#### Scenario: Doubao is late in free band
- **WHEN** `doubao-default` and `gemini-free` both resolve
- **THEN** `gemini` is ranked before `doubao`

#### Scenario: ChatGPT Web remains last resort
- **WHEN** `chatgpt-web-default` and any free API provider are configured
- **THEN** `chatgpt-web` has the lowest autodefault priority

### Requirement: Cost-first model mapping for nano and mini

The gateway SHALL extend `model-mapping.yaml` for `gpt-5.4-nano` and
`gpt-5-mini` so free-tier targets precede paid entries. New mappings SHALL
include, in order before paid fallbacks:

- `longcat/LongCat-Flash-Lite`
- `bazaarlink/auto:free`
- `openrouter/openrouter/free`
- `openrouter/arcee-ai/trinity-large-preview:free`
- `bluesminds/gpt-4.1-nano`
- `sambanova/gpt-oss-120b`
- `ollama-cloud/kimi-k2.6`

Existing free entries (`openrouter/...:free`, `opencode/...`) SHALL remain and
SHALL stay ahead of paid mappings.

#### Scenario: Nano mapping prefers LongCat when configured
- **WHEN** routing `openai/gpt-5.4-nano` through autodefault
- **AND** `longcat-default` resolves
- **THEN** `longcat/LongCat-Flash-Lite` precedes paid `anthropic` mappings

#### Scenario: Mapping skips unavailable providers
- **WHEN** `bazaarlink-default` is not configured
- **THEN** autodefault skips `bazaarlink/auto:free` without error

### Requirement: Capability helpers for new named providers

The gateway SHALL extend `router/capability/providers.rs` with named-provider
helpers for `longcat`, `ollama-cloud`, `sambanova`, `bluesminds`, `bazaarlink`,
`cohere`, `doubao`, and `inclusionai` where YAML metadata alone is insufficient
for runtime capability inference (mirroring `cerebras` / `opencode` patterns).

#### Scenario: JSON-schema routing excludes unsupported models
- **WHEN** a strict JSON-schema request is routed
- **THEN** candidates without `supports-json-schema: true` are excluded
- **AND** `longcat/LongCat-Flash-Thinking` is excluded when `reasoning` disables
  schema support in catalog

### Requirement: Free-tier provider limits for all new providers

The gateway SHALL document conservative `free` tiers in `provider-limits.yaml`
for each new provider with `observed-at`, `scope`, `source`, and per-model or
endpoint RPM/RPD/TPM notes derived from public documentation (June 2026).

Documented monthly-token estimates for prioritization reference:

| Provider | Est. recurring tokens/month |
| --- | ---: |
| `longcat` | 150000000 |
| `doubao` | 60000000 |
| `ollama-cloud` | 20000000 |
| `inclusionai` | 15000000 |
| `sambanova` | 6000000 |
| `bluesminds` | 7200000 |
| `bazaarlink` | 3600000 |
| `cohere` | 800000 |
| `openrouter` (free) | 1200000 |

#### Scenario: Provider limits load for LongCat
- **WHEN** embedded provider limits are loaded
- **THEN** `longcat` defines a `free` tier with daily-token notes for Flash-Lite
  and Flash-Chat families

### Requirement: Credential environment variables

The gateway SHALL resolve each new slot from `AI_GATEWAY_CREDENTIAL_<ID>` with
hyphens mapped to underscores:

| Slot ID | Env var |
| --- | --- |
| `longcat-default` | `AI_GATEWAY_CREDENTIAL_LONGCAT_DEFAULT` |
| `doubao-default` | `AI_GATEWAY_CREDENTIAL_DOUBAO_DEFAULT` |
| `ollama-cloud-default` | `AI_GATEWAY_CREDENTIAL_OLLAMA_CLOUD_DEFAULT` |
| `inclusionai-default` | `AI_GATEWAY_CREDENTIAL_INCLUSIONAI_DEFAULT` |
| `sambanova-default` | `AI_GATEWAY_CREDENTIAL_SAMBANOVA_DEFAULT` |
| `bluesminds-default` | `AI_GATEWAY_CREDENTIAL_BLUESMINDS_DEFAULT` |
| `bazaarlink-default` | `AI_GATEWAY_CREDENTIAL_BAZAARLINK_DEFAULT` |
| `cohere-default` | `AI_GATEWAY_CREDENTIAL_COHERE_DEFAULT` |

#### Scenario: Secrets file supports new slots
- **WHEN** `dev/secrets.local.yaml` includes `credentials.longcat-default.api-key`
- **THEN** slot `longcat-default` resolves at startup

### Requirement: Documentation, tests, and release version

The gateway SHALL document setup for each new provider in `docs/providers.md` and
credential env vars in `docs/credentials.md`. Tests SHALL cover config parsing,
credential gating, autodefault order, capability filtering, Groq free
reclassification, OpenRouter slug presence, and mock dispatch per provider
without live API keys in CI. This capability SHALL ship in release
**`0.3.0-beta.20`**.

#### Scenario: CI verifies autodefault order includes LongCat
- **WHEN** tests build autodefault with all free credentials stubbed
- **THEN** provider priority places `longcat` before `openrouter`

#### Scenario: Contributor reads provider setup
- **WHEN** a contributor opens `docs/providers.md`
- **THEN** each Tier 1 provider has base URL, example model prefix, and env var
