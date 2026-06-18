# Routing and failover

## Autodefault router

In **Sidecar** deployment mode, the gateway auto-creates router `autodefault`
when at least one configured provider has a resolved credential.

- **Strategy:** `budget-aware-capability-after`
- **Decision engine:** enabled
- **Tier cascade:** `free-up` (try cheaper tiers first, escalate to paid when
  slots are full)

Endpoint:

```
POST /router/autodefault/chat/completions
```

Provider priority (lower index = higher priority when multiple are available).
Candidates are sorted by **cost-class** first (`free` → `paid` → `paid-browser`),
then `budget-rank`, then provider index:

| cost-class | rank base | meaning |
|------------|-----------|---------|
| `free` | 0 | $0 marginal API keys and DeepSeek Web |
| `paid` | 200 | Metered / tier-3 API (Anthropic, OpenAI, paid Gemini) |
| `paid-browser` | 300 | ChatGPT Plus/Pro browser session |

Within the `free` band (availability-gated):

1. `opencode`
2. `openrouter`
3. `github-models`
4. `mistral`
5. `groq`
6. `cerebras`
7. `cloudflare`
8. `gemini` (free slots first via `budget-rank`; `gemini-default` is `paid`)
9. `deepseek-web` — only if session file exists

Then `paid`:

10. `anthropic`
11. `openai`

Then `paid-browser`:

12. `chatgpt-web` — **last resort**; only if `CHATGPT_BROWSER_CLI` session file exists

Default example model for autodefault: **`openai/gpt-5.4-nano`** (override with
`AI_GATEWAY_AUTODEFAULT_DEFAULT_MODEL`).

### Intent routing (0.4.1-beta.1)

Autodefault treats the client `model` field as a **routing intent**, not a
binding SKU in `model-mapping.yaml`. Configure with
`source-model-selection: intent` on the router (autodefault sets this
automatically).

| Client model | Intent tier | Floor | Notes |
|--------------|-------------|-------|-------|
| `gpt-5-nano`, `gpt-5-mini` | fast-thinking | fast-thinking (json strict) / fast (plain) | mini ≡ nano |
| plain `gpt-5` | deep | deep | no downgrade to scout |
| other | standard | standard | default |

**Payload shape:**

- `response_format.type = json_schema` (strict) → only upstream with
  `supports-json-schema: true` in the intent band
- plain chat → json-schema and non-json upstream in the fast-thinking band

**Asymmetric stability:** the router may escalate to a higher intent tier when
the preferred band is exhausted, but never downgrade below the client floor.

**Strict mode** (`source-model-selection: strict`, default for named routers):
legacy behaviour — candidates must match `model-mapping.yaml` for the source
model.

Response headers on successful autodefault routes:

```
X-Routing-Intent-Tier: fast-thinking
X-Routing-Selection-Phase: preferred | escalated
```

Route trace logs include `routing_intent_tier` and `routing_selection_phase`.

**Breaking change (beta.17):** ChatGPT Web is no longer the first autodefault
provider when a session file is present. Free API keys are tried first; browser
sessions are last-resort fallbacks.

Payload-aware filtering (beta.16) runs after cost-class ranking. When every
candidate fails the TPM/context filter, the best-effort tail may still jump to
large-context paid providers — see design notes in OpenSpec.

Override per request with header:

```
X-Decision-Tier: free | freemium | paid
```

## Budget-aware selection

The budget-aware router builds a candidate list from **credential slots** (not
just providers). Each candidate carries:

- `credential-id` (for example `openrouter-default`)
- Provider and model after capability checks
- `cost-class` and `budget-rank` from `credentials.yaml`

When an upstream call fails or returns rate-limit signals, the router tries the
next candidate. Multiple credential slots for the same provider and model are
**load-balanced** (round-robin) and grouped for failover before the next
provider. Cooldowns are tracked **per credential id**, so one exhausted
OpenRouter key does not block another slot.

## Structured JSON failover

Requests with OpenAI `response_format.type = json_schema` (strict) are routed
only to providers/models that declare `supports-json-schema: true` in
`providers.yaml`. **`deepseek-web/deepseek-chat`** and
**`deepseek-web/deepseek-reasoner`** are eligible when a browser session is
configured; the mapper injects schema instructions (no native API field).

If the upstream response is invalid or missing expected fields, the router can
fail over to the next capable candidate instead of returning a broken payload.

## Response header: routed identity

Successful responses may include:

```
X-RealMode-Model-And-Provider: {credential-id}/{model}
```

Example: `openrouter-default/gpt-oss-120b:free`

Use this header to see which credential and model actually served the request
after failover.

## Upstream pacing

Before dispatch, the gateway acquires a **pacing permit** derived from
`provider-limits.yaml`:

- **Concurrent** in-flight requests (default 1 if not specified)
- **RPM** (requests per minute)
- **Min interval** between requests (optional)

Pacing is per provider catalog entry; session providers (ChatGPT Web) have
dedicated limits in the same file.

## Cooldowns and 429 handling

`provider-limits.yaml` defines default cooldown durations:

| Class | Default |
|-------|---------|
| Provider error | 15s |
| Rate limit | 60s |
| Quota exhausted | 1h |
| Auth error | 5m |

On 429 responses, the gateway resolves wait time from (in order):

1. `Retry-After` header
2. JSON body hints
3. Error text patterns
4. Defaults above

Classification distinguishes rate-limit vs quota-exhausted for different
fallback durations. Credentials in cooldown are skipped until the window expires.

## Named routers

Custom routers in YAML can use other strategies (`model-latency`, weighted,
etc.). See [`local.yaml`](../ai-gateway/config/local.yaml) for examples.

Path pattern:

```
POST /router/{router-id}/chat/completions
```

## Model mapping

When failover needs an equivalent model on another provider,
[`model-mapping.yaml`](../ai-gateway/config/embedded/model-mapping.yaml) supplies
aliases (for example mapping `gpt-4o-mini` to several cheaper alternatives).

## Related

- [configuration.md](configuration.md)
- [credentials.md](credentials.md)
- [providers.md](providers.md)
