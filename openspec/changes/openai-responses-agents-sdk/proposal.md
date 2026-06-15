## Why

Upstream [#173](https://github.com/Helicone/ai-gateway/issues/173) asks whether the gateway works with **OpenAI Responses API** and **Agents SDK**. The issue body is empty; the title carries a compatibility question. This fork registers **`v1/chat/completions`** only — no **`/v1/responses`** or generic **`/v1/*`** passthrough. Users need an explicit product posture, not silent 404s.

## What Changes

- Record decision **A** (full parity), **B** (bounded surface), **C** (transparent passthrough), or **D** (document-only/defer).
- If A/B/C: compatibility matrix + acceptance tests before implementation.
- Documentation: supported API slice, pinned SDK/API versions, expansion process.
- If D: README must not imply Responses/Agents support.

## Capabilities

### New Capabilities

- `openai-responses-compat`: Responses API and Agents SDK compatibility contract (if A/B/C).

### Modified Capabilities

- `documentation-onboarding`: OpenAI compatibility section and honest marketing boundaries.

## Impact

- OpenAI endpoint module, dispatcher, routing, streaming policy, security (passthrough blast radius if **C**).

**Upstream:** [Helicone/ai-gateway#173](https://github.com/Helicone/ai-gateway/issues/173)

**Migrated from:** `.todos/05-change-request.md`
