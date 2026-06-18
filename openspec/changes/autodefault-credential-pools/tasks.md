## 1. Model-mapping audit and alignment — cancelled

**Skipped:** `autodefault-intent-routing` makes strict `model-mapping.yaml` parity
optional for autodefault; mapping audit deferred.

- [x] 1.1 cancelled — GitHub `gpt-4o-mini` on `gpt-5-mini`
- [x] 1.2 cancelled — Groq scout on nano aliases
- [x] 1.3 cancelled — free-tier prefix alignment
- [x] 1.4 cancelled — CI mapping-audit test
- [x] 1.5 cancelled — Groq scout selection test

## 2. Gemini sixteen-slot catalog

- [x] 2.1 Add `gemini-free-9` … `gemini-free-16` to embedded `credentials.yaml` (`tier: free`, `budget-rank: 0`)
- [x] 2.2 Update `dev/secrets.emulated.yaml` and `dev/secrets.local.example.yaml` with slot stubs
- [x] 2.3 Extend `credential_balance` unit test for sixteen-slot round-robin rotation
- [x] 2.4 Update `docs/credentials.md` — document `gemini-free` through `gemini-free-16`
- [x] 2.5 Add `gemini_sixteen_slot` routing_load scenario (64 concurrent, 16 slots)

## 3. DeepSeek Web two-session pool

- [x] 3.1 Add `deepseek-web-2` to embedded `credentials.yaml` (`provider: deepseek-web`, `tier: free`)
- [x] 3.2 Update `dev/secrets.local.example.yaml` and `dev/secrets.emulated.yaml` with second session-file stub
- [x] 3.3 Add unit test: pacing gates isolated for `deepseek-web-default` vs `deepseek-web-2` (extend `pacing/registry.rs` pattern)
- [x] 3.4 Add unit test: round-robin alternates two DeepSeek Web credential ids
- [x] 3.5 Update `docs/credentials.md` and `docs/deepseek-web.md` for two-session setup

## 4. Docs and validation

- [x] 4.1 Update `docs/providers.md` if Gemini slot count or DeepSeek pool is referenced
- [x] 4.2 `mise exec -- openspec validate autodefault-credential-pools --strict`
- [x] 4.3 Targeted `cargo test` — credential_balance, deepseek pacing, routing_load
- [x] 4.4 `cargo clippy` on touched modules

## 5. Release notes

- [x] 5.1 Add `CHANGELOG.md` entry: Gemini×16, DeepSeek×2 (no mapping parity)
