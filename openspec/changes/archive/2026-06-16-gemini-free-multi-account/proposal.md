## Why

Operators often have **multiple free-tier Google AI Studio API keys** for Gemini, but the gateway only defines **one** free slot (`gemini-free`). Traffic through a single free project hits RPM/RPD limits quickly. Sibling-account round-robin and per-slot cooldown already exist in the router but cannot be used until each key is a first-class credential slot.

Release target after implementation and tests: **`0.3.0-beta.12`** (from `0.3.0-beta.11`). Four free Gemini accounts become peer credential slots; paid `gemini-default` (tier-3) stays unchanged.

## What Changes

- Add three new free-tier Gemini credential slots (`gemini-free-2`, `gemini-free-3`, `gemini-free-4`) alongside the existing `gemini-free` slot.
- Keep all four slots on `tier: free`, equal `budget-rank`, and isolated cooldown/failover state per slot.
- Document env var naming (`AI_GATEWAY_CREDENTIAL_*`) for all four keys.
- Extend credential resolution tests and budget-aware routing tests for four sibling Gemini free accounts.
- Update `.env.template`, `docs/credentials.md`, and `docs/providers.md`.

## Capabilities

### New Capabilities

- `gemini-free-multi-account`: Four free-tier Gemini credential slots, env resolution, autodefault participation, round-robin, and per-slot failover/cooldown.

### Modified Capabilities

- None (no living specs in `openspec/specs/` yet).

## Impact

- `ai-gateway/config/embedded/credentials.yaml` — four free Gemini slots + unchanged `gemini-default`.
- `ai-gateway/src/config/credential_env.rs` — legacy `GEMINI_FREE_TIER_*` env remains mapped only to `gemini-free` (backward compatible).
- Budget-aware router — no algorithm change; more candidates at runtime when env vars are set.
- Provider limits — `gemini` `free` tier metadata already applies to all free slots.
- Workspace version: root `Cargo.toml` → **`0.3.0-beta.12`** after tests pass.
