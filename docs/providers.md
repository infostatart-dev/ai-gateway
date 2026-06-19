# Providers

Provider definitions live in
[`providers.yaml`](../ai-gateway/config/embedded/providers.yaml): base URL, model
list, and per-model capabilities (`supports-tools`, `supports-json-schema`,
`context-window`, etc.).

Model names in requests use the form `{provider}/{model}` (for example
`openai/gpt-4o-mini`, `openrouter/google/gemini-2.0-flash-001`).

## Credential slots (this fork)

Embedded slots in [`credentials.yaml`](../ai-gateway/config/embedded/credentials.yaml):

| Slot ID | Provider | Notes |
|---------|----------|-------|
| `openai-default` | openai | Standard OpenAI API |
| `anthropic-default` | anthropic | Claude API |
| `gemini-free` | gemini | Free-tier Google AI Studio (slot 1) |
| `gemini-free-2` … `gemini-free-16` | gemini | Free-tier Google AI Studio (slots 2–16) |
| `gemini-default` | gemini | Paid / Tier 3 project |
| `groq-default` | groq | Free developer tier (no card) |
| `openrouter-default` | openrouter | Aggregator; slugs must match live catalog |
| `cloudflare-default` | cloudflare | Workers AI; `account_id:token` in secrets |
| `cerebras-default` | cerebras | Cerebras API |
| `mistral-default` | mistral | Mistral experiment tier (~1B tok/mo) |
| `opencode-default` | opencode | OpenCode Free tier |
| `github-models-default` | github-models | GitHub Models PAT (`models:read`) |
| `longcat-default` | longcat | Meituan LongCat public beta |
| `doubao-default` | doubao | Volcengine Ark (cn-beijing) |
| `ollama-cloud-default` | ollama-cloud | Ollama Cloud (not local `ollama`) |
| `inclusionai-default` | inclusionai | InclusionAI free API |
| `sambanova-default` | sambanova | SambaNova free tier |
| `bluesminds-default` | bluesminds | BluesMinds free aggregator |
| `bazaarlink-default` | bazaarlink | BazaarLink `auto:free` routing |
| `cohere-default` | cohere | Cohere trial compatibility API |

Configure keys in [`dev/secrets.local.yaml`](../dev/secrets.local.example.yaml)
(see [credentials.md](credentials.md)). Missing slot secrets are skipped at startup.

## Extended providers (fork highlights)

### OpenRouter

- Base URL: `https://openrouter.ai/api/v1/`
- Model slugs must exist in the [OpenRouter catalog](https://openrouter.ai/api/v1/models)
- Free-tier: `:free` suffix (for example `openai/gpt-oss-120b:free`) or router
  slug `openrouter/free` (auto-picks a capable free model)
- Live free catalog verified periodically; stale slugs are omitted from embedded config

### Tier 1 free API providers (0.3.0-beta.20+)

OpenAI-compatible providers gated on secrets-file credentials. Autodefault priority
(high → low among configured free API keys): `opencode` → `longcat` → `mistral` →
`openrouter` → `github-models` → `bazaarlink` → `bluesminds` → `groq` → …

| Provider | Base URL | Example model |
|----------|----------|---------------|
| `longcat` | `https://api.longcat.chat/openai/` | `longcat/LongCat-2.0-Preview` |
| `bazaarlink` | `https://bazaarlink.ai/api/v1/` | `bazaarlink/auto:free` |
| `bluesminds` | `https://api.bluesminds.com/v1/` | `bluesminds/gpt-4.1-nano` |
| `sambanova` | `https://api.sambanova.ai/v1/` | `sambanova/gpt-oss-120b` |
| `ollama-cloud` | `https://ollama.com/v1/` | `ollama-cloud/glm-4.7` |
| `inclusionai` | `https://api.inclusionai.tech/v1/` | `inclusionai/inclusion-model` |
| `cohere` | `https://api.cohere.com/compatibility/v1/` | `cohere/command-a-03-2025` |
| `doubao` | `https://ark.cn-beijing.volces.com/api/v3/` | `doubao/doubao-pro-32k` |

```bash
curl http://localhost:8080/ai/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"longcat/LongCat-2.0-Preview","messages":[{"role":"user","content":"hi"}]}'
```

### Cloudflare Workers AI

- Models prefixed with `@cf/` (for example `@cf/meta/llama-3.1-70b-instruct`)
- Credential: `AI_GATEWAY_CREDENTIAL_CLOUDFLARE_DEFAULT="account_id:api_token"`

### OpenCode

- Included in autodefault when `opencode-default` credential resolves
- Some models support reasoning; check `model-capabilities` in YAML

### ChatGPT Web

- Browser session provider, not API-key based
- Model: `chatgpt-web/gpt-5.5-instant` (see embedded config)
- Strict JSON schema routing supported
- Setup: [chatgpt-web.md](chatgpt-web.md)

### DeepSeek Web

- Browser session provider (`userToken` from chat.deepseek.com)
- Credential slots: `deepseek-web-default`, `deepseek-web-2` (isolated pacing)
- Models: `deepseek-web/deepseek-chat`, `deepseek-web/deepseek-reasoner`
- Tools not supported initially
- Setup: [deepseek-web.md](deepseek-web.md)

### GitHub Models

- OpenAI-compatible chat completions via GitHub PAT
- Base upstream: `https://models.github.ai/inference/chat/completions`
- Credential: `AI_GATEWAY_CREDENTIAL_GITHUB_MODELS_DEFAULT` (PAT must include **`models:read`** scope)
- Model IDs keep the publisher prefix upstream, for example
  `github-models/openai/gpt-4.1` → upstream body model `openai/gpt-4.1`
- Included in **autodefault** only when `github-models-default` resolves; priority
  is after `openrouter` and `mistral`, before `bazaarlink`
- Embedding IDs (`openai/text-embedding-3-large`, `openai/text-embedding-3-small`) are catalog-only in v1
- Live catalog: [models.github.ai/inference/models](https://models.github.ai/inference/models)

Example request:

```json
{
  "model": "github-models/openai/gpt-4o-mini",
  "messages": [{"role": "user", "content": "Hello"}]
}
```

### Other bundled providers

Also defined in `providers.yaml`: **bedrock**, **ollama**, **xai**, **deepseek**,
**hyperbolic**, and others. Enable by setting credentials (where applicable) and
using the correct model prefix.

Bedrock requires AWS credentials (`AWS_ACCESS_KEY`, `AWS_SECRET_KEY`) and models
must be enabled in your AWS account before use.

## Model mapping

[`model-mapping.yaml`](../ai-gateway/config/embedded/model-mapping.yaml) lists
fallback targets when the requested OpenAI model name should map to equivalents
on other providers during budget-aware / capability routing.

## Provider limits

Operational limits and cooldown hints per provider/tier are in
[`provider-limits.yaml`](../ai-gateway/config/embedded/provider-limits.yaml).
Used for pacing gates and 429 cooldown resolution — see [routing.md](routing.md).

**Quota scope (tier → slot → model):** `provider-limits.yaml` declares
`quota-profile` per provider (`per-model` for Gemini and OpenRouter free,
`per-session` for browser providers). Pacing gates and cooldown keys resolve at
`(credential, upstream_model)` when `per-model`. Gemini and OpenRouter free use
[`provider-ladders.yaml`](../ai-gateway/config/embedded/provider-ladders.yaml)
for intra-slot failover order (fast → capacity → stability → deprioritized)
before inter-slot round-robin.

**OpenRouter per-slug vs shared bucket:** upstream enforces **per-model** daily
quotas on `:free` slugs (e.g. Nemotron exhausted while gpt-oss still works).
The gateway maps each wire slug to its own pacing gate and model-scoped cooldown;
a 402 on a paid slug does not retire `:free` routes on the same credential.

**Adding per-model ladder for another provider:** (1) set `quota-profile:
per-model` in `provider-limits.yaml`, (2) add tier model RPM/RPD rows, (3) add
`provider-ladders.yaml` entry, (4) add routing_load scenario — no router code
changes required.

### Gemini catalog verify (0.4.2-beta.3+)

Embedded Gemini `upstream_slug` values in `providers.yaml` and free-tier ladder
slugs in `provider-ladders.yaml` must exist in the frozen ListModels fixture
[`gemini-listmodels.json`](../ai-gateway/tests/fixtures/gemini-listmodels.json).
Structured entries may split wire vs limits keys, for example:

```yaml
- upstream: gemini-3-flash-preview
  catalog: gemini-3-flash
```

Refresh the fixture after Google catalog changes, then run:

```bash
mise run catalog:verify-gemini
```

### OpenRouter catalog verify (0.4.2-beta.4+)

OpenRouter `upstream_slug` values, free-tier ladder slugs, and per-slug limit keys
must exist in
[`openrouter-listmodels.json`](../ai-gateway/tests/fixtures/openrouter-listmodels.json).

```bash
mise run catalog:verify-openrouter
```

OpenAI-compat providers can follow the same pattern (fixture + verify test) when
their catalogs are curated in embedded YAML.

For **payload-aware routing** (autodefault fat json_schema requests), the
budget-aware router also reads per-model **TPM** caps from this catalog at
filter time: `effective_window = margin(min(context_window, tpm))`. Candidates
that cannot fit the estimated request footprint (`input + reserved max_tokens`)
are skipped before dispatch; unknown limits fail open.

## Request examples

```bash
# OpenAI via gateway unified API
curl http://localhost:8080/ai/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"openai/gpt-4o-mini","messages":[{"role":"user","content":"hi"}]}'

# OpenRouter free model via autodefault router
curl http://localhost:8080/router/autodefault/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"openrouter/openai/gpt-oss-120b:free","messages":[{"role":"user","content":"hi"}]}'
```

## Related

- [credentials.md](credentials.md)
- [configuration.md](configuration.md)
- [chatgpt-web.md](chatgpt-web.md)
