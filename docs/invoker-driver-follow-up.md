# Invoker driver follow-up (out of ai-gateway repo)

Contract for the Graphiti / gateway **invoker driver** that calls
`POST /router/autodefault/chat/completions` (or named routers). Implementation
lives in the invoker repository, not in `ai-gateway`.

Gateway side is complete: `CallerContextLayer` parses headers and
`plan_route_chain` uses `work_unit_id` for spread and route memory.

## Required headers

| Header | Source | Required |
|--------|--------|----------|
| `X-Agent-Name` | stable invoker id (e.g. `graphiti-worker`) | SHOULD |
| `X-Work-Unit-Id` | `session_id` for the chat / structured task | SHOULD when sticky routing desired |
| `Helicone-Session-Id` | same as `session_id` if work-unit header omitted | MAY (fallback) |

Precedence on the gateway: `X-Work-Unit-Id` wins over `Helicone-Session-Id`.

## Task 11.1 — wire `session_id` on every LLM call

On `analyze_structured`, `chat`, and any autodefault completion helper:

```python
headers = {
    "X-Agent-Name": agent_name,
    "Authorization": f"Bearer {gateway_api_key}",
}
if session_id:
    headers["X-Work-Unit-Id"] = session_id
    headers["Helicone-Session-Id"] = session_id  # 11.2 fallback parity
```

```typescript
const headers: Record<string, string> = {
  'X-Agent-Name': agentName,
  Authorization: `Bearer ${apiKey}`,
};
if (sessionId) {
  headers['X-Work-Unit-Id'] = sessionId;
  headers['Helicone-Session-Id'] = sessionId;
}
```

Without `work_unit_id`, route memory and hash spread are disabled for that request.

## Task 11.2 — optional explicit header

Emitting `X-Work-Unit-Id` whenever `session_id` is set is sufficient. Do not
invent per-request random ids — reuse the conversational `session_id` for the
whole work unit.

## Concurrency (11.3)

See [routing.md](routing.md#invoker-concurrency-guidance). Cap parallel LLM calls
per agent to roughly the count of healthy free credential slots from
`GET /v1/observability/provider-stats`.

## Verification

After invoker changes land:

1. Gateway route trace shows `work_unit_id` and `route_memory_hit=true` on repeat calls.
2. `routing_load` scenario `route_memory_sticky_reuse` behaviour matches production.
3. Distinct `session_id` values spread across Gemini free slots under load.

## Tracking

- OpenSpec change: `client-context-route-planning` tasks §11 (contract only in this repo).
- Implementation PR: **invoker / gateway driver repository** (not `ai-gateway`).
