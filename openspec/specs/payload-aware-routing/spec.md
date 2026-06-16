# payload-aware-routing

## Purpose

Pre-flight token estimation and effective-window filtering so autodefault routing
drops upstream candidates that cannot fit large chat payloads (especially
json_schema requests) before the first hop, eliminating guaranteed-dead TPM and
context-overflow failovers.

## Requirements

### Requirement: Request token estimation
The router SHALL estimate the input token count of each chat request using a
provider-aware tokenizer applied to the full serialized prompt, including
message content, tool definitions, and the `response_format` json_schema. The
estimate SHALL be computed once per request from the buffered request body, not
per candidate.

#### Scenario: Fat json_schema payload is estimated including the schema
- **WHEN** a request carries a large `response_format.json_schema` plus message content
- **THEN** the estimated input token count includes the serialized schema and tool definitions
- **AND** the estimate is computed a single time for the whole candidate walk

#### Scenario: Estimate is attached to request requirements
- **WHEN** request requirements are extracted before candidate selection
- **THEN** `min_context_tokens` reflects `estimated_input + reserved_output`
- **AND** is no longer left unset

### Requirement: Output token reservation
The router SHALL reserve output tokens equal to the request `max_tokens` when
present, and a configured default reservation otherwise, and SHALL include this
reservation when checking whether a candidate can accept the request.

#### Scenario: max_tokens present
- **WHEN** a request specifies `max_tokens`
- **THEN** the reserved output equals that value

#### Scenario: max_tokens absent
- **WHEN** a request omits `max_tokens`
- **THEN** the router reserves the configured default output budget

### Requirement: Effective window pre-flight filtering
Before dispatch, the router SHALL drop any candidate whose effective window
cannot fit the request, where `effective_window = min(model context window,
per-model per-minute token cap)` and the candidate is eligible only when
`estimated_input + reserved_output <= effective_window * (1 - safety_margin)`.
The per-model token cap SHALL be sourced from the provider limits catalog.

#### Scenario: Groq over per-minute token cap is skipped pre-flight
- **WHEN** estimated input exceeds a groq model's TPM cap (e.g. 60946 > 30000)
- **THEN** that groq candidate is removed before any upstream call is made
- **AND** no `413 Payload Too Large` hop is produced for it

#### Scenario: OpenRouter over context window is skipped pre-flight
- **WHEN** `estimated_input + reserved_output` exceeds a model's context window (e.g. ~132212 > 131072)
- **THEN** that OpenRouter candidate is removed before dispatch
- **AND** no context-length `400` hop is produced for it

#### Scenario: Candidate that fits is retained
- **WHEN** `estimated_input + reserved_output` fits within a candidate's margin-adjusted effective window
- **THEN** that candidate remains eligible and is dispatched in budget order

### Requirement: Fail-open on unknown limits
The router SHALL filter a candidate only when a concrete context window or token
cap is known and provably exceeded. When a candidate's context window or token
cap is unknown, the router SHALL NOT remove it on payload-size grounds.

#### Scenario: Unknown limit is not filtered
- **WHEN** a candidate has no known context window or token cap
- **THEN** payload-aware filtering does not remove it

### Requirement: Best-effort attempt when no candidate fits
When payload-aware filtering would remove every candidate, the router SHALL
retain the largest-effective-window candidate(s) as a best-effort tail so the
request receives at least one honest upstream attempt rather than an internal
provider-not-found error.

#### Scenario: Oversized request still gets one honest attempt
- **WHEN** the estimated request exceeds every candidate's effective window
- **THEN** the router still dispatches to the largest-window candidate
- **AND** surfaces that upstream's error instead of an opaque internal error
