# caller-request-context Specification

## Purpose
TBD - created by archiving change client-context-route-planning. Update Purpose after archive.
## Requirements
### Requirement: Extract invoker name from inbound headers

The gateway SHALL parse the calling invoker name from the first non-empty value among:

1. `X-Agent-Name`
2. `Helicone-Property-Agent` (value only, without header prefix)

When neither header is present, the gateway SHALL use `unknown-invoker`.

#### Scenario: X-Agent-Name takes precedence

- **WHEN** a router request includes `X-Agent-Name: invoker-alpha`
- **AND** `Helicone-Property-Agent: invoker-beta`
- **THEN** the resolved invoker name is `invoker-alpha`

#### Scenario: Helicone property fallback

- **WHEN** a router request includes only `Helicone-Property-Agent: invoker-gamma`
- **THEN** the resolved invoker name is `invoker-gamma`

#### Scenario: Missing identity defaults unknown

- **WHEN** a router request includes neither invoker header
- **THEN** the resolved invoker name is `unknown-invoker`

### Requirement: Extract work unit id from inbound headers

The gateway SHALL parse an optional work unit identifier from the first non-empty
value among:

1. `X-Work-Unit-Id`
2. `Helicone-Session-Id`

When neither is present, work unit id SHALL be absent (`None`).

#### Scenario: Work unit id header preferred over session id

- **WHEN** a request includes `X-Work-Unit-Id: job-47` and `Helicone-Session-Id: sess-abc`
- **THEN** the resolved work unit id is `job-47`

#### Scenario: Session id without work unit header

- **WHEN** a request includes only `Helicone-Session-Id: unit-48`
- **THEN** the resolved work unit id is `unit-48`

### Requirement: Attach CallerRequestContext to router requests

The gateway SHALL attach a `CallerRequestContext { agent_name, work_unit_id }`
extension to every request that enters a load-balanced router handler before
budget-aware selection runs.

#### Scenario: Context available in failover loop

- **WHEN** autodefault handles a chat completion with `X-Agent-Name` set
- **THEN** `CallerRequestContext` is present in request extensions during candidate planning

### Requirement: Propagate caller context to route trace

The per-request routing trace summary SHALL include `agent_name` and, when
present, `work_unit_id` fields.

#### Scenario: Trace records invoker on success

- **WHEN** a routed request completes successfully with `X-Agent-Name: invoker-alpha`
- **THEN** the emitted route trace includes `agent_name = invoker-alpha`

