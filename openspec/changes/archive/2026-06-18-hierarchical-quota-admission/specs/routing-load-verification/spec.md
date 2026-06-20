## ADDED Requirements

### Requirement: Admission control routing load catalog

The routing load verification catalog SHALL include scenarios proving `quota-admission-control`:

| File | Proves |
|------|--------|
| `admission_zero_repeat_429.rs` | After reconcile blocks scope, no second upstream 429 on same scope |
| `admission_parallel_account_spread.rs` | N work units use N distinct feasible accounts (no pool cap) |
| `admission_hop_readmit.rs` | Re-admit after first 429 routes to sibling without repeat 429 |
| `admission_longcat_tpd.rs` | LongCat TPD from catalog drives infeasible without magic constants |
| `admission_per_session_deepseek.rs` | `deepseek-web-2` session scope admits independently of sibling |

Each scenario SHALL assert `repeat_429_violation` is absent and attempt counts match expected HTTP.

#### Scenario: admission_zero_repeat_429

- **WHEN** first request triggers reconcile block on a `CredentialModel` scope
- **THEN** second concurrent request does not HTTP to that scope
- **AND** `repeat_429_violations` remains 0

#### Scenario: admission_parallel_account_spread with sixteen Gemini secrets

- **WHEN** sixteen `gemini-free*` credentials are configured and feasible
- **AND** eight concurrent work units arrive
- **THEN** at least eight distinct accounts receive first-hop attempts when all are feasible

#### Scenario: admission_per_session_deepseek

- **WHEN** `deepseek-web-default` is infeasible (session cooldown)
- **AND** `deepseek-web-2` is feasible
- **THEN** traffic uses `deepseek-web-2` without inheriting sibling block

---

### Requirement: Unit tests cover three quota profiles

`ai-gateway/tests/quota_admission.rs` SHALL test admission verdicts for `per-model`, `per-slot`, and
`per-session` scopes using catalog fixtures without live API keys.

#### Scenario: Three-profile matrix passes in CI

- **WHEN** `cargo test quota_admission` runs with all features
- **THEN** per-model, per-slot, and per-session admission cases pass
