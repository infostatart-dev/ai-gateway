# work-unit-route-memory

## Purpose

Remember the last successful upstream route per invoker work unit so repeat LLM
calls from the same parallel worker avoid replanning from scratch and return
faster when quota state still permits the prior binding.

## ADDED Requirements

### Requirement: Store last successful route binding per work unit

The gateway SHALL maintain an in-process `WorkUnitRouteMemory` keyed by
`(agent_name, work_unit_id)` storing:

```text
RouteBinding {
  credential_id: ProviderCredentialId,
  model_slug: String,
  recorded_at: Instant,
}
```

Bindings SHALL expire after a configurable TTL (default 30 minutes) since
last successful record. Storage SHALL use in-process `moka` cache (existing workspace
dependency) with TTL eviction — same semantics as Portkey sticky sessions v1
(in-process; Redis deferred to v2).

When `work_unit_id` is absent, route memory SHALL NOT read or write.

#### Scenario: Success records binding

- **WHEN** a request with `X-Agent-Name: invoker-alpha` and `X-Work-Unit-Id: unit-47`
  succeeds on `(gemini-free-9, gemini-3.1-flash-lite)`
- **THEN** route memory stores that binding for `(invoker-alpha, unit-47)`

#### Scenario: No work unit id skips memory

- **WHEN** a request omits work unit headers and succeeds
- **THEN** route memory is unchanged

### Requirement: Planner prefers viable remembered binding

The route chain planner SHALL place a remembered binding as the **first hop** when
route memory contains a binding for the request's caller context and all hold:

1. Credential is not circuit-open
2. Model is ladder-eligible for the request
3. `QuotaSnapshot` reports headroom for `(credential, model)`
4. Binding model is not below routing intent floor

The planner SHALL NOT treat the binding as an exclusive lock; failover MAY continue
through subsequent planned hops if the binding fails.

#### Scenario: Second call reuses binding

- **WHEN** `unit-47` previously succeeded on `gemini-free-9/gemini-3.1-flash-lite`
- **AND** a follow-up request arrives with the same agent and work unit id
- **AND** quota snapshot shows headroom on that binding
- **THEN** the first planned hop is `gemini-free-9/gemini-3.1-flash-lite`
- **AND** route trace reports `route_memory_hit=true`

#### Scenario: Expired binding ignored

- **WHEN** a binding TTL has elapsed
- **THEN** the planner does not prefer the expired binding
- **AND** route trace reports `route_memory_hit=false`

### Requirement: Invalidate binding on failoverable failure

The gateway SHALL remove a remembered binding from route memory before replanning
when a failoverable upstream failure occurs on a hop that matches the binding for
the request's work unit.

#### Scenario: 429 on sticky route invalidates memory

- **WHEN** `unit-47` has a remembered binding on `gemini-free-9`
- **AND** the next request fails with HTTP 429 on that binding
- **THEN** route memory no longer contains a binding for `unit-47`
- **AND** route trace reports `route_memory_invalidated=true`
- **AND** the replanned first hop is not `gemini-free-9` when circuit-open or zero headroom

#### Scenario: Success refreshes TTL

- **WHEN** a binding exists and the same binding succeeds again
- **THEN** `recorded_at` is updated
- **AND** TTL restarts

### Requirement: Route memory observability

The per-request route trace SHALL include `route_memory_hit` and
`route_memory_invalidated` boolean fields.

#### Scenario: Trace reports memory miss on first call

- **WHEN** a work unit has no prior binding
- **THEN** route trace reports `route_memory_hit=false` and `route_memory_invalidated=false`
