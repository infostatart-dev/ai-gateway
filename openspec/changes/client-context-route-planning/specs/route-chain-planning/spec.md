# route-chain-planning

## Purpose

Build a short, scored upstream hop chain per inbound request instead of linear
failover across the full autodefault candidate pool. Spread parallel work units
across healthy credential slots using live quota headroom, work-unit route memory,
and embedded free-tier catalog ladders.

## ADDED Requirements

### Requirement: Plan short route chain before upstream walk

Before the failover loop executes, the budget-aware router SHALL call
`plan_route_chain` to produce an ordered list of at most **7** `BudgetCandidate`
entries (configurable constant in code, not operator YAML in v1).

The failover loop SHALL attempt only planned candidates in order. When the plan is
exhausted without success, the router SHALL rebuild the plan once excluding failed
hops. If the rebuilt plan is empty, the router SHALL return terminal failure.

#### Scenario: Successful request within plan length

- **WHEN** the first planned candidate succeeds
- **THEN** upstream attempts equal 1
- **AND** no candidate outside the plan is called

#### Scenario: Plan rebuild after exhaustion

- **WHEN** all candidates in the initial plan fail with failoverable errors
- **AND** at least one viable candidate was excluded from the first plan only due to ordering
- **THEN** the router rebuilds the plan once before returning terminal failure

#### Scenario: Plan caps hop count

- **WHEN** a request would previously walk more than 7 upstream candidates
- **THEN** the first plan contains at most 7 candidates
- **AND** provider-stats `upstream_attempts` for that inbound request is at most 7 plus one rebuild pass (≤14 absolute ceiling in v1)

### Requirement: Exclude circuit-open, zero-headroom, and zero-success providers from plan

Route planning SHALL omit credentials in circuit-open state.

Route planning SHALL omit candidates with `QuotaSnapshot.headroom_score == 0.0` per
`quota-headroom-scheduling`.

Route planning SHALL omit providers whose rolling health window shows zero successes
and at least 10 attempts since process start (pod-lifetime dead provider filter).

#### Scenario: Cloudflare excluded after zero successes

- **WHEN** `cloudflare-default` has 50 attempts and 0 successes since gateway start
- **THEN** no plan includes `cloudflare-default`

#### Scenario: Healthy OpenRouter included

- **WHEN** `openrouter-default` has success rate above 50%
- **THEN** the plan may include an openrouter hop before paid-browser providers

### Requirement: Caller-aware credential spread among healthy slots

The planner MUST, when multiple healthy credentials exist for the same
`(provider, upstream_model)` pool and `CallerRequestContext.work_unit_id` is
present, choose primary credential order by:

```text
stable_hash(agent_name, work_unit_id) % healthy_credentials.len()
```

rotating the healthy credential list to put the selected credential first.

When `work_unit_id` is absent, the gateway SHALL retain existing round-robin
behavior.

#### Scenario: Three parallel work units spread across Gemini keys

- **WHEN** three concurrent autodefault requests arrive with the same `X-Agent-Name`
- **AND** work unit ids `unit-1`, `unit-2`, `unit-3`
- **AND** `gemini-free-9`, `gemini-free-10`, and `openrouter-default` are healthy
- **AND** `gemini-free-2` through `gemini-free-8` are circuit-open
- **THEN** each request's first planned Gemini hop uses a different healthy credential when possible
- **AND** no plan's first hop uses a circuit-open credential

#### Scenario: No work unit id preserves round-robin

- **WHEN** requests omit work unit headers
- **THEN** credential ordering within a pool uses the existing round-robin counter

### Requirement: Stability escalation UP within plan before cross-provider hop

The planner MUST, for Gemini free per-model ladders (`provider-ladders.yaml`),
when preferred-band models on a credential are unavailable (cooldown, circuit, or
quota exhausted per snapshot), append ladder hops **upward** on the **same**
credential in order:

1. **capacity** band (`gemini-3.1-flash-lite`, `gemini-2.5-flash`)
2. **stability** band (`gemini-2.5-flash-lite`)

before switching to a different provider.

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

### Requirement: Integrate work-unit route memory in plan construction

The route chain planner SHALL insert a viable `work-unit-route-memory` binding as
hop 0 per that capability's rules when the caller context provides one.

When no binding exists or binding is not viable, planning proceeds with hash spread
and ladder construction only.

#### Scenario: Memory binding leads plan

- **WHEN** route memory contains a viable binding for the work unit
- **THEN** the first entry in the planned chain matches the binding

### Requirement: Plan observability fields

The route trace SHALL record `planned_hops` (plan length before walk),
`plan_rebuilds` (count of replan invocations), `route_memory_hit`, and
`route_memory_invalidated`.

#### Scenario: Trace includes plan metadata

- **WHEN** a request plans 5 hops and succeeds on hop 2
- **THEN** route trace reports `planned_hops=5` and `upstream_attempts=2`
