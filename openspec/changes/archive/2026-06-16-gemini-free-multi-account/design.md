## Context

- Embedded credentials today: `gemini-free` (tier `free`, rank 0) and `gemini-default` (tier `tier-3`, rank 10).
- Use case: spread autodefault Gemini traffic across **four** free AI Studio keys before falling through to other providers.
- Budget-aware router already supports multiple credential slots per provider via `CredentialRoundRobin` and per-`ProviderCredentialId` cooldown in `ProviderState`.
- Release version for this change: **`0.3.0-beta.12`** (bump after implementation + tests).

## Goals / Non-Goals

**Goals:**

- Register **four** free Gemini credential slots in embedded config.
- Resolve each slot only from its own `AI_GATEWAY_CREDENTIAL_<ID>` env var (plus existing legacy aliases for `gemini-free` only).
- Participate in autodefault when the slot secret is present; skip silently when missing.
- Round-robin requests across configured free slots; on 429/quota, cool down **that slot only** and try siblings before leaving Gemini.
- Tests proving 4-slot registration, env resolution, round-robin order, and sibling failover.

**Non-Goals:**

- Paid tier-3 slot changes (`gemini-default` stays as-is).
- Dynamic / unbounded slot discovery from env prefixes (no `GEMINI_FREE_*` wildcard scanner in v1).
- Operator-specific deployment wiring (how env vars reach the process is outside this repo).
- Changing Gemini model catalog or provider-limits numbers.

## Decisions

### 1. Slot IDs and env vars

Add explicit slots (kebab-case id → env):

| Slot ID | Environment variable |
|---------|---------------------|
| `gemini-free` | `AI_GATEWAY_CREDENTIAL_GEMINI_FREE` (unchanged) |
| `gemini-free-2` | `AI_GATEWAY_CREDENTIAL_GEMINI_FREE_2` |
| `gemini-free-3` | `AI_GATEWAY_CREDENTIAL_GEMINI_FREE_3` |
| `gemini-free-4` | `AI_GATEWAY_CREDENTIAL_GEMINI_FREE_4` |

**Rationale:** Matches existing `AI_GATEWAY_CREDENTIAL_<ID>` convention; no magic numbering in code.  
**Alternative rejected:** Single env with comma-separated keys — breaks per-slot cooldown identity and observability.

### 2. Tier and budget-rank

All four slots: `tier: free`, `budget-rank: 0`.

**Rationale:** Equal priority; round-robin distributes load. Paid `gemini-default` remains rank 10 and is tried after free siblings exhaust or cooldown.

### 3. Legacy env aliases

Keep `GEMINI_FREE_TIER_API_KEY` / `GEMINI_FREE_TIER_APIKEY` **only** on `gemini-free`.

**Rationale:** Avoid ambiguous mapping when multiple free keys exist.

### 4. Autodefault inclusion

No change to `autodefault_provider_order()` logic: Gemini joins autodefault when **any** Gemini credential resolves. With four free slots + optional `gemini-default`, all configured slots become budget-aware candidates.

### 5. Cooldown scope

Reuse existing per-`ProviderCredentialId` state (already scoped in pacing/routing tests as `gemini-free/...`).

**Rationale:** No new cooldown machinery required.

## Risks / Trade-offs

- **[Risk] Four keys from the same egress IP still share upstream abuse/rate limits** → Mitigation: document that keys should be separate AI Studio projects where possible.
- **[Risk] Partial configuration (only 2 of 4 env vars set)** → Mitigation: skip missing slots at startup (existing behavior); router uses whatever resolved.
- **[Risk] Operators run an old binary against new env var names** → Mitigation: extra vars are ignored on older releases; rollback drops unused slots.

## Migration Plan

1. Implement four slots + tests; run `cargo test` and `cargo clippy` on touched crates.
2. Bump workspace version in root `Cargo.toml` to **`0.3.0-beta.12`**.
3. Publish release **`0.3.0-beta.12`** (crate + container tags per existing project CI).
4. Operators set all four `AI_GATEWAY_CREDENTIAL_GEMINI_FREE*` env vars before restart.
5. Verify under load: round-robin across configured free slots; sibling failover on 429.
6. Rollback: redeploy **`0.3.0-beta.11`**; extra env vars and slots are ignored by the old binary.

## Open Questions

- Should we add a startup INFO line listing resolved Gemini free slot IDs (count + ids) for operability?
- After four free keys stabilize, should `gemini-default` leave autodefault entirely (free-only profile)?
