## MODIFIED Requirements

### Requirement: Plan short route chain before upstream walk

Before the failover loop executes, the budget-aware router SHALL call
`plan_route_chain` to produce an ordered list of at most **7** `BudgetCandidate`
entries (configurable constant in code, not operator YAML in v1).

The failover loop SHALL attempt only planned candidates in order. When the plan is
exhausted without success, the router SHALL rebuild the plan once excluding failed
hops using a **fresh** quota snapshot (not the stale plan-time snapshot). If the
rebuilt plan is empty, the router SHALL return terminal failure.

Before each upstream attempt, the router SHALL re-peek quota oracle for that
candidate and skip non-callable hops without HTTP.

#### Scenario: Successful request within plan length

- **WHEN** the first planned candidate succeeds
- **THEN** upstream attempts equal 1
- **AND** no candidate outside the plan is called

#### Scenario: Plan rebuild after exhaustion

- **WHEN** all candidates in the initial plan fail with failoverable errors
- **AND** at least one viable candidate was excluded from the first plan only due to ordering
- **THEN** the router rebuilds the plan once with a fresh snapshot before returning terminal failure

#### Scenario: Plan caps hop count

- **WHEN** a request would previously walk more than 7 upstream candidates
- **THEN** the first plan contains at most 7 candidates
- **AND** provider-stats `upstream_attempts` for that inbound request is at most 7 plus one rebuild pass (≤14 absolute ceiling in v1)

#### Scenario: Hop skipped when oracle turns non-callable

- **WHEN** a planned hop was callable at plan time
- **AND** re-peek before dispatch shows `next_wait > 0`
- **THEN** the hop is skipped without HTTP
- **AND** provider-stats shows no attempt for that pair on that inbound request

---

### Requirement: Stability escalation UP within plan before cross-provider hop

The planner MUST, for Gemini free per-model ladders (`provider-ladders.yaml`),
when preferred-band models on a credential are unavailable (cooldown, circuit, or
quota exhausted per snapshot), append ladder hops **upward** on the **same**
credential in order:

1. **capacity** band (`gemini-3.1-flash-lite`, `gemini-2.5-flash`)
2. **stability** band (`gemini-2.5-flash-lite`)

before switching to a different provider.

Only ladder models with `headroom_score > 0` at plan time SHALL be appended.
The walk SHALL re-validate headroom before each ladder hop.

Stability escalation MUST NOT:

- Select models below the routing intent floor defined by `autodefault-intent-routing`
- Downgrade to a faster/smaller model on another provider when a stability-band model
  on a healthy Gemini slot still has quota headroom
- Select openrouter **deprioritized** models (e.g. nemotron) while Gemini stability
  band has headroom on any healthy slot

#### Scenario: Fast band exhausted escalates to flash-lite on same slot

- **WHEN** a fast-thinking json request plans against `gemini-free-9`
- **AND** fast-band models on that slot have zero quota headroom
- **AND** `gemini-3.1-flash-lite` on the same slot has positive headroom
- **THEN** the plan includes the flash-lite hop before any openrouter hop

#### Scenario: Stability band before cross-provider

- **WHEN** fast and capacity models on `gemini-free-9` are exhausted
- **AND** `gemini-2.5-flash-lite` on that slot has headroom
- **THEN** the plan includes `gemini-2.5-flash-lite` before openrouter

#### Scenario: Floor prevents downgrade to fast-only pool

- **WHEN** routing intent floor is `fast-thinking`
- **THEN** the plan SHALL NOT select upstream whose intent tier is below `fast-thinking` except the existing fast-band widening for plain (non-json) requests per intent spec

#### Scenario: Never downgrade model on failover

- **WHEN** a planned hop fails on `gemini-3.1-flash-lite`
- **THEN** replan excludes failed hop
- **AND** replan SHALL NOT insert `gemini-3-flash-preview` on another provider as the next hop

#### Scenario: Ladder omits zero-headroom intermediate model

- **WHEN** fast-band model has zero headroom
- **AND** next ladder model also has zero headroom
- **AND** stability-band model has positive headroom
- **THEN** the plan jumps to the stability-band hop
- **AND** does not include HTTP attempts for zero-headroom ladder models
