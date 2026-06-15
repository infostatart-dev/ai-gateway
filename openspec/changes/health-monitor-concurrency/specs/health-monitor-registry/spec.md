## ADDED Requirements

### Requirement: Documented concurrency direction before registry refactor

The project SHALL record an explicit concurrency direction (A: retain `RwLock` baseline, or B: finer-grained concurrency) before merging structural changes to the health-monitor registry.

#### Scenario: PR proposes DashMap or alternate container

- **WHEN** a pull request changes the health-monitor map container or lock strategy
- **THEN** `openspec/changes/health-monitor-concurrency/design.md` records the chosen direction and rationale
- **AND** reviewers reject the PR if direction **B** is chosen without documented async safety invariants

### Requirement: Async safety invariants when direction B is chosen

If direction **B** is selected, the project SHALL document library-agnostic invariants prohibiting collect-keys → `await` → re-fetch without a stable handle for the duration of a check.

#### Scenario: Health tick spans await points

- **WHEN** a health monitor tick iterates registry entries and performs async I/O
- **THEN** the design document lists invariants each implementation must satisfy
- **AND** acceptance scenarios cover registration during an in-flight tick
