## 1. Credential catalog

- [x] 1.1 Add `gemini-free-2`, `gemini-free-3`, `gemini-free-4` to `ai-gateway/config/embedded/credentials.yaml` (`tier: free`, `budget-rank: 0`)
- [x] 1.2 Confirm `gemini-default` unchanged; verify embedded parse tests still pass

## 2. Env resolution

- [x] 2.1 Keep legacy `GEMINI_FREE_TIER_*` aliases scoped to `gemini-free` only in `credential_env.rs`
- [x] 2.2 Add unit tests for `AI_GATEWAY_CREDENTIAL_GEMINI_FREE_2` … `_4` resolution and skip-when-empty behavior

## 3. Routing behavior

- [x] 3.1 Extend budget-aware tests: four free Gemini candidates round-robin on repeated selection
- [x] 3.2 Extend credential failover tests: 429 on `gemini-free` tries `gemini-free-2` without shared cooldown
- [x] 3.3 Verify autodefault builds Gemini candidates when any free slot resolves (existing logic; add regression test if missing)

## 4. Docs and templates

- [x] 4.1 Update `.env.template` with four `AI_GATEWAY_CREDENTIAL_GEMINI_FREE*` entries
- [x] 4.2 Update `docs/credentials.md` and `docs/providers.md` (four free slots table, env setup)

## 5. Version bump and release (`0.3.0-beta.12`)

- [x] 5.1 Run scoped tests/clippy for credential + budget-aware changes
- [x] 5.2 Bump root `Cargo.toml` workspace version **`0.3.0-beta.11` → `0.3.0-beta.12`**
- [ ] 5.3 Confirm CI passes (Rust tests + Docker publish per existing workflows)
