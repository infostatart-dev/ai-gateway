## Decision status

**Status:** pending  
**Owner:** (assign)

## Options

| Option | Summary |
|--------|---------|
| **A** | Full Responses + Agents SDK as tested compatibility targets |
| **B** | Bounded surface (Responses + explicit allowlist) |
| **C** | Generic `/v1/*` passthrough with explicit policy |
| **D** | Document-only / defer with hard boundaries |

## Expected end state

- Written decision A/B/C/D referenced to #173
- If A/B/C: answer “yes, for …” with matrix + test recipe, or “no, because …” for unsupported combos
- If D: “no / not today” with clear boundaries and workarounds
- No silent partial support (no production surprise 404 on `/v1/responses`)

## Notes

- Do not invent reproduction details from empty issue body — anchor on pinned SDK/API versions.
- Full OpenAI platform (Assistants, Files, Batch, etc.) needs separate change unless explicit phase 2.
- **C** increases abuse surface — pair with allowlist and size caps.
