## Context

GitHub Models exposes an OpenAI-compatible chat-completions API at
`https://models.github.ai/inference/chat/completions`. Access is account-scoped
via a GitHub personal access token with `models:read`. Model IDs use a
`{publisher}/{model_name}` shape (for example `openai/gpt-4.1`).

The gateway already separates provider catalog entries from credential slots.
`github-models` fits this model: one catalog entry serves many upstream model
IDs; each GitHub token is a separate credential slot for failover, cooldown, and
pacing.

Provider id `github-models` must remain distinct from any future or external
GitHub Copilot OAuth integration (`github`).

Release version for this change: **`0.3.0-beta.15`** (from `0.3.0-beta.14`).

## Goals / Non-Goals

**Goals:**

- Add `github-models` as a first-class provider on the OpenAI-compatible
  dispatcher path with GitHub-specific static headers.
- Keep auth in the existing `AI_GATEWAY_CREDENTIAL_<ID>` convention.
- Preserve upstream model IDs exactly (`openai/gpt-4.1`, not `gpt-4.1`).
- Register the curated chat catalog with per-model context windows and
  conservative capability metadata.
- Include `github-models` in autodefault when the default credential resolves.
- Verify behavior with unit and mock integration tests, without a live PAT in CI.

**Non-Goals:**

- No browser or OAuth login flow.
- No live GitHub API calls in CI.
- No embeddings dispatch in v1 (catalog entries only).
- No automatic live model sync from `https://models.github.ai/inference/models`
  in the first pass.
- No paid GitHub Models / Azure migration path in this change.

## Decisions

### 1. Provider id and routing shape

Use provider id `github-models`. Request model ids use
`github-models/{publisher}/{model}`; only the first path segment is the gateway
provider prefix. Upstream body field `model` keeps the publisher prefix
(for example `openai/gpt-4.1`).

### 2. OpenAI-compatible dispatch with static headers

GitHub Models accepts standard chat-completions JSON. Implementation extends
embedded provider config with optional `request-headers` (or an equivalent
thin client) so `X-GitHub-Api-Version` and `Accept` are always sent. No request
body mapper is expected unless tests prove otherwise.

Static headers (from GitHub Models inference API):

| Header | Value |
| --- | --- |
| `Authorization` | `Bearer <PAT>` |
| `X-GitHub-Api-Version` | `2022-11-28` |
| `Accept` | `application/vnd.github+json` |

PAT must include scope **`models:read`**.

### 3. Default credential slot

Add `github-models-default`:

- provider: `github-models`
- tier: `free`
- budget-rank: `0` (same band as other free API-key slots)
- env: `AI_GATEWAY_CREDENTIAL_GITHUB_MODELS_DEFAULT`

Operators may add more slots later (`github-models-alt`, etc.) for additional
GitHub accounts.

### 4. Curated static model catalog

Chat models (upstream IDs and default context windows):

| Model ID | Context |
| --- | ---: |
| `openai/gpt-4.1` | 1047576 |
| `openai/gpt-4o` | 128000 |
| `openai/gpt-4o-mini` | 128000 |
| `openai/o1` | 200000 |
| `openai/o3` | 200000 |
| `openai/o4-mini` | 200000 |
| `deepseek/DeepSeek-R1` | 131072 |
| `meta/Llama-4-Maverick-17B-128E-Instruct` | 131072 |
| `xai/grok-3` | 131072 |
| `mistral-ai/Mistral-Medium-3` | 128000 |
| `cohere/Cohere-command-a` | 128000 |
| `microsoft/Phi-4` | 16384 |

Embedding IDs listed for catalog completeness only (no v1 dispatch):

- `openai/text-embedding-3-large`
- `openai/text-embedding-3-small`

Live catalog reference for operators:
`https://models.github.ai/inference/models`.

### 5. Conservative capability metadata (v1)

| Model ID | tools | json-schema | reasoning |
| --- | --- | --- | --- |
| `openai/gpt-4.1` | yes | yes | no |
| `openai/gpt-4o` | yes | yes | no |
| `openai/gpt-4o-mini` | yes | yes | no |
| `openai/o1` | no | no | yes |
| `openai/o3` | yes | yes | yes |
| `openai/o4-mini` | yes | yes | yes |
| `deepseek/DeepSeek-R1` | no | no | yes |
| `meta/Llama-4-Maverick-17B-128E-Instruct` | yes | yes | no |
| `xai/grok-3` | yes | no | no |
| `mistral-ai/Mistral-Medium-3` | yes | yes | no |
| `cohere/Cohere-command-a` | yes | yes | no |
| `microsoft/Phi-4` | yes | no | no |

Tighten flags only after live validation; loosening is safer than false positives
for budget-aware / JSON-schema routing.

### 6. Autodefault policy

Include `github-models` in `autodefault` **only when** `github-models-default`
resolves (same gating pattern as `opencode`). Priority order insertion:

`… → openrouter → github-models → mistral → …`

### 7. Provider limits (conservative v1)

GitHub publishes per-model rate tiers (Low / High / Embedding / specialized).
High-tier chat models are typically ~10 RPM, ~50 RPD, 2 concurrent requests;
limits also vary with Copilot plan.

v1 embedded limits use a single conservative `free` tier for the provider:

- `rpm: 10`
- `rpd: 50`
- `concurrent: 2`
- scope: `api-key`
- cooldown: `provider-error: 30s`, standard rate-limit backoff

Document in notes that per-model tiers on GitHub's side may be stricter and
that operators must respect GitHub Acceptable Use Policy (no reselling /
abusive automation).

## Risks / Trade-offs

- GitHub may change model IDs, headers, or scopes → keep IDs in one YAML block;
  document `models:read` and catalog URL.
- Free quotas are account- and model-tier-scoped → conservative limits + per-slot
  cooldown; expect 429s on hot models.
- Capability flags are heuristic until probed → conservative defaults.
- `github` name collision in tooling → always use `github-models` in config, env,
  metrics, and docs.

## Migration Plan

1. Extend provider config schema for static `request-headers` (if not present).
2. Add `github-models` to `providers.yaml` with models and capabilities.
3. Add `github-models-default` to `credentials.yaml` and `.env.template`.
4. Add `provider-limits.yaml` entry and autodefault order in `config/read.rs`.
5. Wire dispatcher headers; add mock upstream tests.
6. Document setup in `docs/providers.md` (or dedicated short doc).
7. Bump workspace version **`0.3.0-beta.14` → `0.3.0-beta.15`**.
8. Validate OpenSpec and run targeted Rust tests.

## Open Questions

- Should per-model rate tiers (Low vs High vs Embedding) be encoded in
  `provider-limits.yaml` in a follow-up once we have probe data?
- Should embeddings get a separate endpoint change after chat routing is stable?
