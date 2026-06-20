## MODIFIED Requirements

### Requirement: Plan short route chain before upstream walk

Before the failover loop executes, the budget-aware router SHALL call `plan_route_chain` to
produce an ordered list of at most **7** `BudgetCandidate` entries.

The failover loop SHALL attempt only planned candidates that pass hop-time re-admission. When the
plan is exhausted without success, the router SHALL rebuild the plan once with a **fresh**
`QuotaSnapshot`. If the rebuilt plan is empty, the router SHALL return terminal failure.

#### Scenario: Successful request within plan length

- **WHEN** the first feasible planned candidate succeeds
- **THEN** upstream attempts equal 1
- **AND** no candidate outside the plan is called

#### Scenario: Plan rebuild uses fresh admission state

- **WHEN** all candidates in the initial plan fail with failoverable errors
- **THEN** the router rebuilds the plan once with a new snapshot before terminal failure

#### Scenario: Hop skipped when admission changes after plan

- **WHEN** a planned hop was feasible at plan time
- **AND** re-admission before dispatch shows infeasible
- **THEN** the hop is skipped without HTTP

---

### Requirement: Stability escalation UP within plan before cross-provider hop

The planner MUST append ladder hops **upward** on the **same** account only when each ladder model
passes admission at plan time. The walk SHALL re-admit before each ladder hop.

Only feasible ladder models SHALL be appended. Stability escalation rules from
`autodefault-intent-routing` remain unchanged.

#### Scenario: Fast band infeasible escalates to feasible flash-lite same account

- **WHEN** fast-band models on `gemini-free-9` are infeasible
- **AND** `gemini-3.1-flash-lite` on the same account is feasible
- **THEN** the plan includes flash-lite before any cross-provider hop

#### Scenario: Ladder omits infeasible intermediate models

- **WHEN** fast-band and capacity-band models are infeasible
- **AND** stability-band model is feasible
- **THEN** the plan includes only the feasible stability-band hop
