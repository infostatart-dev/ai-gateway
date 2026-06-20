## MODIFIED Requirements

### Requirement: Admission verdict is feasible only when all dimensions clear

The gateway SHALL consult shared Redis state for the resolved `PacingScope` when
`upstream_pacing.store` is `redis` and the distributed pacing backend is healthy, covering
pacing peek, daily headroom, cooldowns, and reconcile blocks before returning `feasible`.

When Redis is disabled or degraded, feasibility SHALL use local `PacingGate` state only.

#### Scenario: Distributed RPM block marks infeasible on all replicas

- **GIVEN** Redis store is enabled
- **AND** shared counters show RPM wait greater than zero for a `CredentialModel` scope
- **WHEN** any replica evaluates admission for that scope
- **THEN** `feasible` is false
- **AND** `blocked_reason` identifies the limiting dimension
