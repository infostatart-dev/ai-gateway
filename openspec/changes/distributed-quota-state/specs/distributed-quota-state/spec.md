## ADDED Requirements

### Requirement: Pacing scopes map to shared Redis keys

When `upstream_pacing.store` is `redis`, the gateway SHALL persist pacing and reconcile state
under Redis keys derived from `pacing_scope_key(PacingScope)` — the same key function used by
local `PacingRegistry` gates.

#### Scenario: Per-model scope key is distinct per slug

- **GIVEN** provider `gemini` has `quota-profile: per-model`
- **WHEN** replica A acquires for `CredentialModel { gemini-free-3, gemini-3-flash-preview }`
- **THEN** Redis key differs from `gemini-free-3::gemini-3.1-flash-lite`
- **AND** both keys are independent quota buckets

#### Scenario: Session scope uses session path key

- **GIVEN** provider `deepseek-web` has `quota-profile: per-session`
- **WHEN** distributed store resolves scope for `deepseek-web-default`
- **THEN** Redis key equals the configured session file path string
- **AND** sibling credentials with distinct session files do not share the key

### Requirement: Distributed acquire enforces shared global quota

When Redis store is enabled, `PacingGate::acquire` and `peek_next_wait` SHALL use atomic
check-and-increment (or equivalent) against Redis so the sum of concurrent attempts across
replicas does not exceed catalog `concurrent` / RPM / daily limits for that scope.

#### Scenario: Two replicas share concurrent limit

- **GIVEN** catalog `concurrent: 1` for a scope
- **AND** replica A holds an acquired permit in Redis
- **WHEN** replica B calls `peek_next_wait` for the same scope
- **THEN** wait is greater than zero
- **AND** admission marks the scope infeasible without HTTP

### Requirement: Reconcile blocks replicate across replicas

The gateway MUST write `apply_upstream_reconcile(until)` results to Redis for the scope key
so all replicas treat the scope infeasible until `until` without requiring another upstream 429.

#### Scenario: Reconcile on one replica blocks peers

- **GIVEN** replica A applies reconcile until T on `(openrouter-default, nemotron slug)`
- **WHEN** replica B evaluates admission milliseconds later
- **THEN** `feasible` is false for that scope
- **AND** replica B skips HTTP on the scope

### Requirement: Local-only fallback when Redis unavailable

When `upstream_pacing.store` is `local` or Redis is unreachable, the gateway SHALL use
existing in-process `PacingGate` behavior without failing inbound HTTP startup.

#### Scenario: Redis disabled preserves Phase 1 semantics

- **WHEN** `upstream_pacing.store` is `local`
- **THEN** admission uses only process-local gates
- **AND** behavior matches `quota-admission-control` Phase 1 requirements
