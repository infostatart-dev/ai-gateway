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

## Caller context and route planning (0.5.0.1)

Autodefault and budget-aware routers read inbound **caller context** headers for
deterministic hop-0 spread, work-unit sticky routing, and observability.

### Invoker header contract

| Header | Role |
|--------|------|
| `X-Agent-Name` | Invoker identity (preferred) |
| `Helicone-Property-Agent` | Fallback agent name |
| `X-Work-Unit-Id` | Sticky route memory key (preferred) |
| `Helicone-Session-Id` | Fallback work-unit id when explicit id absent |
| `X-Request-Id` | Synthetic work-unit when no session/explicit id (per-request spread) |

**Work-unit resolution ladder** (router routes always get a non-empty id):

1. `X-Work-Unit-Id` → `work_unit_source: explicit`
2. `Helicone-Session-Id` → `helicone-session`
3. `X-Request-Id` (set by the HTTP stack) → `request-id`
4. Generated UUID v4 → `generated` (only when step 3 is absent)

Route trace logs include `work_unit_source`. When the source is `request-id` or
`generated`, the gateway may echo the resolved id in response header
`X-Work-Unit-Id` (default on for router routes).

When `work_unit_id` is present, the planner may reuse a recent successful
`(credential, model)` binding for the same `(agent_name, work_unit_id)` pair.
Binding failures (429, auth, quota) invalidate memory for that pair.

### Sticky memory vs parallel spread (FAQ)

**Sticky memory is intentional:** the same `(agent_name, work_unit_id)` prefers
the last successful `(credential, model)` on hop 0. This is ideal for sequential
turns in one chat session.

**Parallel calls sharing one work unit** compete for the same sticky binding and
pacing permits — cap concurrency per session or assign distinct work units
(`session_id` per task).

**Anonymous parallel traffic** without session headers still spreads: each request
with a distinct `X-Request-Id` resolves to a different synthetic work unit.
Multi-turn sticky across requests requires an explicit session header
(`X-Work-Unit-Id` or `Helicone-Session-Id`).

### Stability escalation order

Within a credential slot, the planner walks the **model ladder** upward before
cross-provider failover:

1. **Fast** band (`gemini-3.1-flash-lite`, fast OpenRouter free models, …)
2. **Capacity** band
3. **Stability** band (`gemini-2.5-flash-lite`, …)
4. **Deprioritized** band (e.g. Nemotron) only when higher bands lack headroom

The router never downgrades below the client intent floor.

### Invoker concurrency guidance

When sending `X-Work-Unit-Id` / session id:

- Treat each work unit as **one conversational lane** — concurrent calls with the
  same id compete for the same sticky binding and pacing permits.
- Limit parallel LLM calls per agent to roughly the count of **healthy free
  credential slots** with headroom (see `GET /v1/observability/provider-stats`).
- Distinct work units (`session_id` per chat/task) improve spread across Gemini
  free slots and reduce 429 collisions.

### Provider-stats routing health

`GET /v1/observability/provider-stats` includes a per-credential
`routing_health` object:

| Field | Meaning |
|-------|---------|
| `circuit_open` | Credential circuit breaker is active |
| `open_until` | Estimated wall-clock reopen time (when open) |
| `success_rate` | Rolling 5-minute success ratio |
| `planner_excluded` | Credential skipped by route planner (circuit or dead slot) |

### Route trace and replay

Terminal route logs include `planned_hops`, `plan_rebuilds`, `route_memory_hit`,
`work_unit_source`, and a structured **`ReplayRecord`** with hop-0 score breakdown
(`h_success`, `quota_capacity`, `hash_bias`, …) for incident analysis without
prompt bodies. JSON field `q_headroom` is a deprecated alias for `quota_capacity`.

### Invoker driver follow-up (out of repo)

Implementation checklist: [invoker-driver-follow-up.md](invoker-driver-follow-up.md).

The Graphiti / gateway **invoker driver** SHOULD:

1. Pass `session_id` as `X-Work-Unit-Id` on `analyze_structured` / `chat` calls.
2. Optionally mirror the same value in `Helicone-Session-Id`.
3. Cap concurrent structured calls per session to the healthy free-slot estimate.

## Related

- [configuration.md](configuration.md)
- [credentials.md](credentials.md)
- [providers.md](providers.md)
