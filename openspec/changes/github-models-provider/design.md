## Context

The gateway already has a clean separation between provider catalog entries and credential slots. GitHub Models fits this shape: one provider entry can serve many model IDs, while each GitHub token is represented as a separate credential slot for failover, cooldown, and pacing.

The provider should be treated as an external OpenAI-compatible service with GitHub-specific headers, not as an aggregator alias or a generic OpenAI base URL override.

## Goals / Non-Goals

**Goals:**

- Add `github-models` as a first-class provider.
- Keep auth in the existing `AI_GATEWAY_CREDENTIAL_<ID>` convention.
- Preserve GitHub model IDs exactly as upstream expects them.
- Include enough capability metadata to make budget-aware and structured-output routing predictable.
- Verify behavior with unit and mock integration tests, without requiring a live GitHub token in CI.

**Non-Goals:**

- No browser/session login flow.
- No live GitHub API calls in CI.
- No search, embeddings, or non-chat endpoints in the initial implementation.
- No automatic live model sync in the first implementation pass.

## Decisions

1. Use provider id `github-models`.

   This avoids overloading the existing `github` OAuth/Copilot meaning and keeps model routing explicit: `github-models/openai/gpt-4.1`.

2. Use the OpenAI-compatible dispatcher path.

   GitHub Models accepts a chat-completions style request. The implementation should only add a provider catalog entry and any required static headers unless tests prove a mapper is needed.

3. Use a dedicated default credential slot.

   Add `github-models-default` with provider `github-models`, tier `free`, and budget rank near other free-first providers. Operators can add more slots later for additional GitHub accounts.

4. Start with a curated static model list.

   Include the models needed for routing value first: `openai/gpt-4.1`, `openai/gpt-4o`, `openai/gpt-4o-mini`, `openai/o1`, `openai/o3`, `openai/o4-mini`, `deepseek/DeepSeek-R1`, `meta/Llama-4-Maverick-17B-128E-Instruct`, `xai/grok-3`, `mistral-ai/Mistral-Medium-3`, `cohere/Cohere-command-a`, and `microsoft/Phi-4`.

## Risks / Trade-offs

- GitHub may change model IDs or token scopes -> document the scope and keep model IDs in one catalog entry.
- Free quotas are account-scoped and may vary -> apply conservative provider-limit defaults and rely on per-credential cooldown.
- Some models may not support tools or strict JSON equally -> capability metadata must be conservative until tested.
- The `github` name is already overloaded in developer tooling -> keep the provider id as `github-models` everywhere.

## Migration Plan

1. Add provider/catalog/credential config.
2. Add docs and environment template entries.
3. Add mock upstream tests for required headers and request path.
4. Add routing/capability tests for model selection.
5. Validate OpenSpec and run the smallest relevant Rust test slice.

## Open Questions

- Should embeddings be added in a separate follow-up once chat routing is stable?
- Should `github-models` be included in `autodefault` immediately, or gated behind explicit credential presence and a conservative priority?
