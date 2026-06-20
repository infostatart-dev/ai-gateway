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

The gateway SHALL resolve a work unit identifier using this ladder (first non-empty
wins):

1. `X-Work-Unit-Id` → `work_unit_source = explicit`
2. `Helicone-Session-Id` → `work_unit_source = helicone-session`
3. `X-Request-Id` → `work_unit_source = request-id`
4. Generated UUID v4 → `work_unit_source = generated` (only when step 3 is absent)

The resolved value SHALL always be present on router requests (never absent on router
routes). `CallerRequestContext` SHALL include `work_unit_source`.

#### Scenario: Work unit id header preferred over session id

- **WHEN** a request includes `X-Work-Unit-Id: job-47` and `Helicone-Session-Id: sess-abc`
- **THEN** the resolved work unit id is `job-47`
- **AND** `work_unit_source` is `explicit`

#### Scenario: Session id without work unit header

- **WHEN** a request includes only `Helicone-Session-Id: unit-48`
- **THEN** the resolved work unit id is `unit-48`
- **AND** `work_unit_source` is `helicone-session`

#### Scenario: Request id synthetic work unit

- **WHEN** a router request has no work-unit or session headers
- **AND** `X-Request-Id: req-abc-123` is present
- **THEN** the resolved work unit id is `req-abc-123`
- **AND** `work_unit_source` is `request-id`

#### Scenario: Generated fallback when request id absent

- **WHEN** a router request has no work-unit, session, or request-id headers
- **THEN** the gateway assigns a non-empty generated work unit id
- **AND** `work_unit_source` is `generated`

### Requirement: Attach CallerRequestContext to router requests

The gateway SHALL attach a `CallerRequestContext { agent_name, work_unit_id,
work_unit_source }` extension to every request that enters a load-balanced router
handler before budget-aware selection runs.

#### Scenario: Context available in failover loop

- **WHEN** autodefault handles a chat completion with `X-Agent-Name` set
- **THEN** `CallerRequestContext` is present in request extensions during candidate planning
- **AND** `work_unit_id` is non-empty

### Requirement: Propagate caller context to route trace

The per-request routing trace summary SHALL include `agent_name`, `work_unit_id`, and
`work_unit_source`.

#### Scenario: Trace records invoker on success

- **WHEN** a routed request completes successfully with `X-Agent-Name: invoker-alpha`
- **THEN** the emitted route trace includes `agent_name = invoker-alpha`
- **AND** includes `work_unit_id` and `work_unit_source`

### Requirement: Deploy header contract documentation

The repository SHALL document in `docs/routing.md` and the release CHANGELOG upgrade
notes which headers invokers SHOULD send:

| Header | Purpose |
|--------|---------|
| `X-Agent-Name` | Invoker identity for attribution and spread salt |
| `X-Work-Unit-Id` | Conversational / task lane for sticky memory (preferred) |
| `Helicone-Session-Id` | Fallback work-unit id when explicit header omitted |

The documentation SHALL state that synthetic `request-id` work units enable per-request
spread but do **not** replace session id for multi-turn sticky routing.

#### Scenario: Operator reads deploy guide

- **WHEN** an operator opens `docs/routing.md` caller-context section
- **THEN** they see the header table and synthetic fallback behaviour

