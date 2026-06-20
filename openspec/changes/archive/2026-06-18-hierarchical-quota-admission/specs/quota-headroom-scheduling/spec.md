## MODIFIED Requirements

### Requirement: Quota snapshot at plan time

Before building a route chain, the gateway SHALL construct a `QuotaSnapshot` from `QuotaAdmission`
verdicts for each candidate `PacingScope` and catalog-resolved limits:

1. Per-scope `PacingGate` state (RPM, TPM, RPD/TPD, concurrent) from `provider-limits.yaml`
2. Credential health circuit state
3. Active model, slot, and session cooldown timers
4. Estimated request tokens from payload budget
5. Active upstream reconcile blocks

The snapshot SHALL expose `headroom_score(scope) -> f64` where `0.0` means **not feasible**
(any positive `next_wait`, exhausted daily headroom, cooldown, or reconcile block).

#### Scenario: RPD-exhausted model scores zero

- **WHEN** daily window shows no headroom for `gemini-3-flash-preview` on `gemini-free-8`
- **THEN** `headroom_score` for that `CredentialModel` scope is `0.0`

#### Scenario: Feasible scope scores positive

- **WHEN** admission reports `feasible == true` for a scope
- **THEN** `headroom_score` is greater than `0.0`

#### Scenario: Sub-second RPM wait scores zero

- **WHEN** `peek_next_wait` returns any duration greater than zero
- **THEN** `headroom_score` is `0.0`

---

### Requirement: Planner excludes zero-headroom candidates

Route chain planning SHALL omit any candidate whose admission verdict is not feasible from the
initial plan and from replan passes.

Omission SHALL NOT increment provider-stats attempt counters.

The planner SHALL NOT sleep to probe infeasible candidates during plan construction.

#### Scenario: Saturated model skipped without HTTP

- **WHEN** `gemini-3-flash-preview` on `gemini-free-9` is infeasible in the snapshot
- **AND** `gemini-3.1-flash-lite` on the same account is feasible
- **THEN** the plan includes the flash-lite hop
- **AND** provider-stats shows no new attempt on preview for that inbound request

---

### Requirement: Headroom-aware parallel work unit spread

The planner SHALL assign first-hop accounts for concurrent requests with distinct work unit ids
using:

1. Filter to accounts with feasible admission for the target model pool
2. Stable hash spread among feasible survivors; work unit ids matching `unit-<N>` (positive integer
   suffix) MAY use ordinal `N-1` modulo pool size for deterministic parallel routing_load tests
3. When feasible account count is less than concurrent work units, remaining units use the next
   best feasible hop (alternate account or provider), never an infeasible account

The gateway SHALL NOT cap the number of accounts per provider in code; spread uses all configured
credentials with secrets that pass admission.

#### Scenario: Three work units across N feasible Gemini accounts

- **WHEN** three concurrent requests arrive with work units `unit-1`, `unit-2`, `unit-3`
- **AND** five `gemini-free*` accounts are feasible for the preferred model
- **THEN** each request's first hop uses a distinct account when at least three are feasible

#### Scenario: Headroom updates between calls

- **WHEN** `unit-1` exhausts RPM on `gemini-free-9`
- **AND** `unit-2` plans milliseconds later
- **THEN** `unit-2` snapshot reflects infeasibility on `gemini-free-9`
