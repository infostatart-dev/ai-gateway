## ADDED Requirements

### Requirement: Client access config controls inbound key enforcement

The gateway SHALL add a `client-access` config block that enables or disables
first-party inbound API key enforcement, points to the access YAML file, defines
reload behavior, sets maximum protected request body size, and selects the quota
state backend.

#### Scenario: Disabled client access preserves compatibility

- **WHEN** `client-access.enabled` is false or unset
- **THEN** the gateway does not require the new client access YAML file
- **AND** existing non-client-access authentication behavior remains unchanged

#### Scenario: Enabled client access requires valid initial file

- **WHEN** `client-access.enabled` is true
- **AND** the configured client access YAML file is missing or invalid at startup
- **THEN** the gateway fails startup with a client access config error

#### Scenario: Enabled client access is authoritative

- **WHEN** `client-access.enabled` is true
- **AND** `helicone.features` is `auth` or `all`
- **THEN** inbound protected route authentication uses the client access registry
- **AND** Helicone control-plane key state is not consulted for that request

### Requirement: Client access YAML declares subjects, plans, keys, and scopes

The client access YAML file SHALL contain `version: 1`, optional `subjects`, a
required `plans` map, and a `keys` map. Each key entry SHALL reference a subject
and plan, contain a hash-only inbound key identifier, status, and explicit
scopes.

#### Scenario: Valid registry loads

- **WHEN** the YAML contains one active key with `hash: sha256:<hex>`, a valid
  subject reference, a valid plan reference, and scope `unified-api`
- **THEN** the gateway builds a client access snapshot containing that key

#### Scenario: Raw inbound key is rejected

- **WHEN** a key entry contains a raw `api-key`, `token`, or other unrecognized
  secret field instead of `hash`
- **THEN** YAML validation fails
- **AND** no snapshot is built from that file

#### Scenario: Unknown plan is rejected

- **WHEN** a key entry references a plan id not present in `plans`
- **THEN** YAML validation fails
- **AND** the error identifies the unresolved plan reference

### Requirement: Live reload keeps last valid snapshot

The gateway SHALL poll the configured client access YAML file at the configured
reload interval, validate the complete file, and atomically publish a new
snapshot only after successful validation.

#### Scenario: Valid reload revokes a key

- **WHEN** the running snapshot contains active key `old-key`
- **AND** the YAML is replaced by a valid file without `old-key`
- **THEN** requests using `old-key` are rejected after the reload is applied

#### Scenario: Invalid reload does not break traffic

- **WHEN** the running snapshot contains active key `stable-key`
- **AND** the YAML is replaced by syntactically invalid YAML
- **THEN** requests using `stable-key` continue to authenticate against the last
  valid snapshot
- **AND** the gateway records a reload failure log or metric

#### Scenario: Deleted file is treated as invalid reload

- **WHEN** the client access file is deleted after startup
- **THEN** the gateway keeps serving with the last valid snapshot
- **AND** the gateway records a reload failure log or metric

### Requirement: ClientAccessContext is attached to authenticated requests

For every protected request authenticated by client access, the gateway SHALL
attach `ClientAccessContext` to request extensions with key id, subject id,
user id, org id, plan id, scopes, and effective quota limits. Legacy
`AuthContext` MAY be derived for downstream compatibility, but client access and
quota checks SHALL use `ClientAccessContext`.

#### Scenario: Context contains key identity

- **WHEN** a request authenticates with active key `acme-main`
- **THEN** request extensions contain `ClientAccessContext.key_id = acme-main`
- **AND** request extensions contain the user and org ids resolved from the
  referenced subject

#### Scenario: Suspended key is rejected

- **WHEN** a request authenticates with a key whose status is `suspended`
- **THEN** the gateway returns HTTP 401
- **AND** no upstream provider request is attempted

#### Scenario: Expired key is rejected

- **WHEN** a request authenticates with a key whose `expires-at` is before the
  request time
- **THEN** the gateway returns HTTP 401
- **AND** no quota counters are consumed

### Requirement: Route scopes are enforced before dispatch

The gateway SHALL enforce route scopes after route classification and before
router, unified API, or direct proxy dispatch. Supported scopes SHALL include
`unified-api`, `router:<router-id>`, `direct:<provider-id>`, and `*`.

#### Scenario: Unified API scope allows unified request

- **WHEN** a key has scope `unified-api`
- **AND** the request targets `/ai/chat/completions`
- **THEN** the request proceeds to quota admission

#### Scenario: Missing router scope is denied

- **WHEN** a key has scope `router:default`
- **AND** the request targets `/router/private/chat/completions`
- **THEN** the gateway returns HTTP 403
- **AND** no quota counters are consumed

#### Scenario: Wildcard scope allows direct proxy

- **WHEN** a key has scope `*`
- **AND** the request targets a direct provider proxy route
- **THEN** the request proceeds to quota admission

### Requirement: Request quotas cover minute, day, and week windows

For each authenticated key, the gateway SHALL enforce configured request limits
over rolling 60-second, UTC day, and ISO week windows before dispatching a
protected request.

#### Scenario: Request per minute exhausted

- **WHEN** a key plan has `requests.per-minute: 2`
- **AND** two protected requests for that key have been admitted in the current
  rolling 60-second window
- **THEN** the next protected request returns HTTP 429
- **AND** the response contains `retry-after`

#### Scenario: Request per day exhausted

- **WHEN** a key plan has `requests.per-day: 1`
- **AND** one protected request for that key has already been admitted in the
  current UTC day window
- **THEN** the next protected request returns HTTP 429
- **AND** the retry time points to the next daily reset

#### Scenario: Different keys have isolated request quotas

- **WHEN** key `a` has exhausted its request quota
- **AND** key `b` is under its request quota
- **THEN** key `b` requests continue to be admitted

### Requirement: Token quotas reserve and reconcile usage

For quota-protected chat requests, the gateway SHALL buffer the request body up
to `client-access.max-body-bytes`, estimate input tokens, reserve output tokens,
and reserve total estimated tokens before dispatch. The gateway SHALL reconcile
the reservation against reported usage when response usage is available.

#### Scenario: Token per minute exhausted before dispatch

- **WHEN** a key plan has `tokens.per-minute: 100`
- **AND** a protected chat request estimates 120 total tokens
- **THEN** the gateway returns HTTP 429 before upstream dispatch
- **AND** no upstream provider request is attempted

#### Scenario: Successful response commits reported usage

- **WHEN** a protected chat request reserves 100 estimated tokens
- **AND** the upstream response reports total usage of 60 tokens
- **THEN** the quota store commits 60 tokens for that key
- **AND** refunds the unused 40 reserved tokens

#### Scenario: Usage over reservation becomes debt

- **WHEN** a protected chat request reserves 100 estimated tokens
- **AND** the upstream response reports total usage of 130 tokens
- **THEN** the quota store commits 130 tokens for that key
- **AND** future admission considers the extra 30 tokens consumed

#### Scenario: Request body exceeds protected body limit

- **WHEN** a protected chat request body exceeds `client-access.max-body-bytes`
- **THEN** the gateway rejects the request before dispatch
- **AND** no request or token quota counters are consumed

### Requirement: Streaming responses settle quota reservations

The gateway SHALL wrap streaming response bodies for quota-protected requests so
token reservations are committed or refunded when the stream completes or fails.

#### Scenario: Streaming success commits usage

- **WHEN** a protected streaming chat response completes successfully
- **AND** usage is reported in the final stream event or usage header
- **THEN** the token reservation is committed using reported usage

#### Scenario: Streaming error refunds before usage

- **WHEN** a protected streaming request is admitted
- **AND** the stream fails before any upstream usage can be observed
- **THEN** the token reservation is refunded or settled to zero usage

### Requirement: Redis quota backend is authoritative across replicas

When `client-access.quota-store.type` is `redis`, the gateway SHALL use Redis as
the authoritative quota state for request counters, token counters, and
reservations. Admission SHALL check all applicable dimensions atomically.

#### Scenario: Two replicas share request quota

- **WHEN** replica A admits a request for key `shared`
- **AND** the shared key reaches `requests.per-minute`
- **THEN** replica B rejects the next request for key `shared` with HTTP 429
  before dispatch

#### Scenario: Redis reserve checks all dimensions atomically

- **WHEN** a request would fit request windows but exceed token windows
- **THEN** the Redis quota operation rejects the request
- **AND** no partial request counter increment remains for that rejected request

#### Scenario: Redis unavailable fails closed

- **WHEN** `quota-store.type` is `redis`
- **AND** Redis is unavailable during quota admission
- **THEN** the gateway returns HTTP 503 for protected traffic
- **AND** no upstream provider request is attempted

### Requirement: Memory quota backend is single-process only

When `client-access.quota-store.type` is `memory`, the gateway SHALL enforce the
same observable quota semantics within the current process and SHALL not claim
cross-replica quota correctness.

#### Scenario: Memory backend enforces local quota

- **WHEN** `quota-store.type` is `memory`
- **AND** a single gateway process admits requests for a key until its quota is
  exhausted
- **THEN** later requests for the same key in the same process return HTTP 429

#### Scenario: Production warning for memory backend

- **WHEN** `client-access.enabled` is true
- **AND** `quota-store.type` is `memory`
- **AND** deployment target or config indicates production/cloud operation
- **THEN** the gateway emits a startup warning that memory quota is not
  multi-replica safe

### Requirement: Errors and rate limit headers are client compatible

The gateway SHALL return OpenAI-shaped error bodies for client access failures
and SHALL include rate-limit headers on admitted or rejected quota-protected
requests when a limit dimension is known.

#### Scenario: Invalid key returns authentication error

- **WHEN** a protected request has an unknown Bearer key
- **THEN** the gateway returns HTTP 401
- **AND** the response body uses the gateway's OpenAI-compatible error format

#### Scenario: Scope denial returns authorization error

- **WHEN** a protected request authenticates successfully but lacks the required
  route scope
- **THEN** the gateway returns HTTP 403
- **AND** the response body identifies an authorization failure

#### Scenario: Quota rejection includes limit headers

- **WHEN** a protected request is rejected for quota exhaustion
- **THEN** the response includes `retry-after`
- **AND** the response includes rate-limit limit and remaining headers for the
  blocking dimension

### Requirement: Client access observability is exposed

The gateway SHALL expose logs and metrics for client access auth attempts,
rejections, scope denials, quota admissions, quota rejections, Redis quota
errors, and reload success/failure.

#### Scenario: Reload failure is observable

- **WHEN** a live reload fails validation
- **THEN** the gateway emits a structured log or metric with the reload failure
  reason

#### Scenario: Quota rejection is observable without key leakage

- **WHEN** a request is rejected for quota exhaustion
- **THEN** metrics include the key id, plan id, and blocking dimension
- **AND** metrics do not include the raw inbound API key
