## Context

Autodefault already supports:

- Per-credential round-robin (`CredentialRoundRobin`) for same `(provider, model)`
- Per-session pacing scope for web providers (`gate_scope_key` → session path)
- Eight Gemini free slots in embedded catalog (living spec still says four)
- Model mapping per logical alias (`gpt-5-mini`, `gpt-5.4-nano`, …)

Stage profile: clients report `openai/gpt-5-mini` with strict `json_schema`,
target **6 parallel** analyses. OpenRouter absorbs ~94% of traffic on one key;
Gemini×8, Groq scout, and DeepSeek Web matter on failover and fat payloads.

Exploration confirmed:

- Switching the client-reported model to `gpt-5.4-nano` without mapping fix
  **drops Groq** from json_schema candidates (`llama-3.1-8b-instant` fails
  capability filter).
- Second ChatGPT session is **not** desired; ChatGPT stays single last-resort.
- DeepSeek Web at priority #8 (`cost-class: free`) is higher leverage than
  ChatGPT pool expansion.

## Goals / Non-Goals

**Goals:**

- Align `gpt-5-mini`, `gpt-5.4-nano`, and `gpt-5.4-mini` mappings for json_schema
  and GitHub eligibility without forcing a client model change.
- Expand Gemini free credential catalog to **16** slots with no behavior change
  beyond pool size.
- Add **`deepseek-web-2`** session slot; 2× parallel DeepSeek when traffic
  reaches web failover.
- Regression tests proving mapping parity and pool rotation.

**Non-Goals:**

- ChatGPT Web multi-session (explicitly single session forever in v1).
- OpenRouter second account (separate change if primary RPM becomes bottleneck).
- Client-side model config changes — ops guidance only, out of gateway scope.
- opencode autodefault inclusion (still hard-excluded in `read.rs`).

## Decisions

### D1 — One change, four spec deltas (no new capability folders)

**Choice:** Single change `autodefault-credential-pools` modifying four living
specs instead of three parallel changes.

**Rationale:** Shared theme (credential pool + binding audit); avoids triplicate
proposal/design overhead.

**Rejected:** Three separate changes (`gemini-16`, `deepseek-pool`, `mini-binding`).

### D2 — GitHub on `gpt-5-mini`, not client model flip

**Choice:** Insert `github-models/openai/gpt-4o-mini` into `gpt-5-mini` mapping
after free OpenRouter block, before `groq/meta-llama/llama-4-scout-17b-16e-instruct`.

**Rationale:** Stage clients report `gpt-5-mini`; GitHub already in autodefault
priority #3; zero client config change.

**Rejected:** Mandate `gpt-5.4-nano` from clients first (Groq regression without D3).

### D3 — Groq scout for nano and mini aliases

**Choice:** Replace `groq/llama-3.1-8b-instant` with
`groq/meta-llama/llama-4-scout-17b-16e-instruct` in `gpt-5.4-nano` and
`gpt-5.4-mini` mappings to match `gpt-5-mini` and Groq structured-output docs.

**Rationale:** Capability helper only allows scout (and gpt-oss) for json_schema;
mini alias already uses scout.

### D4 — Gemini slots `gemini-free-9` … `gemini-free-16`

**Choice:** Extend embedded `credentials.yaml` with eight new slots; same
`tier: free`, `budget-rank: 0`, `cost-class: free`.

**Rationale:** Mechanical extension; round-robin and cooldown code is
slot-agnostic.

**Rejected:** Dynamic/unlimited slot discovery — breaks explicit catalog policy.

### D5 — DeepSeek: exactly two slots (`default` + `-2`)

**Choice:** Add `deepseek-web-2` only (user confirmed two free sessions, not three).

**Rationale:** 2× `concurrent:1` → 2 parallel DeepSeek completions; pacing
gates already isolate by `session-file` path (see `pacing/registry.rs` test).

**Rejected:** Generic `deepseek-web-{n}` pattern without catalog entries.

### D6 — Mapping audit as CI guard

**Choice:** Add unit test or `routing_load` assertion that `gpt-5-mini` and
`gpt-5.4-nano` share the same free-tier prefix through GitHub/Groq scout entries
(ordered subset equality for first N free mappings).

**Rationale:** Prevents future nano/mini drift discovered on stage.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| 16 Gemini keys from same org/IP share abuse signals | Document: one key per Google Cloud project; ops checklist |
| 2 DeepSeek sessions on one pod IP trigger auth cooldown | Stagger logins; monitor `provider-stats` auth-error |
| GitHub still rarely hit if OpenRouter succeeds first | Expected; mapping makes GitHub eligible without forcing traffic |
| Living `gemini-free-multi-account` spec says "four slots" | MODIFIED requirement in this change; archive sync updates main spec |

## Migration Plan

1. Ship gateway with new catalog slots (missing secrets → skipped, no error).
2. Stage: add `gemini-free-9`…`16` and `deepseek-web-2` to secrets when keys ready.
3. No client restart required for Gemini/DeepSeek pool — gateway reload only.
4. Optional: after deploy, verify `GET /v1/observability/provider-stats` shows
   rotation across new slots under forced Gemini failover test.

Rollback: remove new secrets entries; old 8 Gemini + 1 DeepSeek behavior restored.

## Open Questions

- Whether stage should add Tier-1 API keys (`bazaarlink`, `bluesminds`) in the
  same rollout (secrets-only, no code) — ops decision, not blocking this change.
