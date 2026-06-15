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

Provider priority (lower index = higher priority when multiple are available):

1. `chatgpt-web` — only if `CHATGPT_BROWSER_CLI` session file exists
2. `opencode` → `openrouter` → `mistral` → `groq` → `cerebras` → `cloudflare`
   → `gemini` → `anthropic`

Startup banner shows default policy tier, cascade mode, and fallback chain.
Override per request with header:

```
X-Decision-Tier: free | freemium | paid
```

## Budget-aware selection

The budget-aware router builds a candidate list from **credential slots** (not
just providers). Each candidate carries:

- `credential-id` (for example `openrouter-default`)
- Provider and model after capability checks
- `budget-rank` from `credentials.yaml`

When an upstream call fails or returns rate-limit signals, the router tries the
next candidate. Multiple credential slots for the same provider and model are
**load-balanced** (round-robin) and grouped for failover before the next
provider. Cooldowns are tracked **per credential id**, so one exhausted
OpenRouter key does not block another slot.

## Structured JSON failover

Requests with OpenAI `response_format.type = json_schema` (strict) are routed
only to providers/models that declare `supports-json-schema: true` in
`providers.yaml`.

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
