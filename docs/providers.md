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
| `gemini-free-2` | gemini | Free-tier Google AI Studio (slot 2) |
| `gemini-free-3` | gemini | Free-tier Google AI Studio (slot 3) |
| `gemini-free-4` | gemini | Free-tier Google AI Studio (slot 4) |
| `gemini-default` | gemini | Paid / Tier 3 project |
| `groq-default` | groq | Groq inference |
| `openrouter-default` | openrouter | Aggregator; slugs must match live catalog |
| `cloudflare-default` | cloudflare | Workers AI; env `account_id:token` |
| `cerebras-default` | cerebras | Cerebras API |
| `mistral-default` | mistral | Mistral API |
| `opencode-default` | opencode | OpenCode Free tier |
| `github-models-default` | github-models | GitHub Models PAT (`models:read`) |

Set the matching `AI_GATEWAY_CREDENTIAL_*` env var for each slot you enable.
For Gemini free tier you can configure up to four AI Studio keys
(`AI_GATEWAY_CREDENTIAL_GEMINI_FREE` through `_4`) to spread autodefault traffic
before falling back to `gemini-default` or other providers.
Details: [credentials.md](credentials.md).

## Extended providers (fork highlights)

### OpenRouter

- Base URL: `https://openrouter.ai/api/v1/`
- Model slugs must exist in the [OpenRouter catalog](https://openrouter.ai/api/v1/models)
- Free-tier models often use the `:free` suffix (for example
  `openai/gpt-oss-120b:free`)

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
- Models: `deepseek-web/deepseek-chat`, `deepseek-web/deepseek-reasoner`
- Tools not supported initially
- Setup: [deepseek-web.md](deepseek-web.md)

### GitHub Models

- OpenAI-compatible chat completions via GitHub PAT
- Base upstream: `https://models.github.ai/inference/chat/completions`
- Credential: `AI_GATEWAY_CREDENTIAL_GITHUB_MODELS_DEFAULT` (PAT must include **`models:read`** scope)
- Model IDs keep the publisher prefix upstream, for example
  `github-models/openai/gpt-4.1` → upstream body model `openai/gpt-4.1`
- Included in **autodefault** only when `github-models-default` resolves; priority is after `openrouter`, before `mistral`
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
