## ADDED Requirements

### Requirement: Upstream failure signal taxonomy

The gateway SHALL classify upstream failures into a normalized
`UpstreamFailureKind` at the provider adapter boundary before router policy runs.
The taxonomy SHALL distinguish **events** (what happened) from **implementations**
(how it is encoded on the wire or returned to API clients).

At minimum the gateway SHALL support:

- `RateLimited` — transient throughput cap; cooldown MAY use Retry-After (implementation).
- `CredentialRestricted` — credential/account temporarily blocked from completions;
  cooldown MAY use an upstream-provided `restricted_until` timestamp (implementation).
- `CredentialInvalid` — session or API key no longer accepted.
- Existing classes (`QuotaExhausted`, `Overload`, generic upstream errors) SHALL remain unchanged.

Provider adapters SHALL emit `CredentialRestricted` without requiring the router to
parse provider-specific field names (e.g. DeepSeek `biz_code`, `mute_until`).

#### Scenario: Event is distinct from HTTP status

- **WHEN** an upstream returns HTTP 200 with a business-layer restriction payload
- **THEN** the adapter emits `UpstreamFailureKind::CredentialRestricted`
- **AND** the dispatcher maps it to HTTP 403 for the API client
- **AND** the router applies slot restriction policy without interpreting DeepSeek JSON

#### Scenario: restricted_until is cooldown implementation only

- **WHEN** a provider supplies a future `restricted_until` (or equivalent) timestamp
- **THEN** the adapter attaches it to the `CredentialRestricted` event
- **AND** the router cooldown duration SHALL be `restricted_until - now` (plus buffer)
- **AND** absence of a timestamp SHALL fall back to catalog `credential-restriction` cooldown

### Requirement: Client HTTP mapping for credential restriction

The gateway SHALL map `UpstreamFailureKind::CredentialRestricted` dispatch failures
to HTTP **403 Forbidden** for OpenAI-compatible clients, including `error.code`:
**`credential_restricted`**, a human-readable `error.message`, and optional
`error.restricted_until` (RFC3339). The gateway SHALL NOT return HTTP 502 empty
response for credential restriction.

#### Scenario: DeepSeek mute maps to credential_restricted

- **WHEN** DeepSeek Web completion returns account mute with a known `mute_until`
- **THEN** the client receives HTTP 403 with `error.code=credential_restricted`
- **AND** `error.restricted_until` reflects the upstream mute deadline

#### Scenario: Final autodefault failure surfaces restriction

- **WHEN** all failover candidates are exhausted and the last failure was credential restriction
- **THEN** the client receives HTTP 403 with `error.code=credential_restricted`
- **AND** not HTTP 502 empty response

### Requirement: Router policy for credential restriction

The router SHALL classify `UpstreamFailureKind::CredentialRestricted` (from
response extension or HTTP 403 `credential_restricted` body) as
`FailoverClass::CredentialRestricted` with `ExhaustionScope::Slot`, set cooldown to
`restricted_until - now` when present otherwise catalog
`credential-restriction + retry-after-buffer`, insert the credential into
`failed_credentials`, and SHALL NOT retry the same credential on structured-output
or executor turn retries.

#### Scenario: Restricted slot is skipped for remainder of walk

- **WHEN** `deepseek-web-default` returns credential restriction
- **AND** `deepseek-web-2` is configured and healthy
- **THEN** the router attempts `deepseek-web-2` next
- **AND** does not attempt `deepseek-web-default` again in the same request

#### Scenario: No short provider-error retry loop

- **WHEN** a credential restriction occurs without `restricted_until`
- **THEN** cooldown is at least the catalog `credential-restriction` duration for that provider
- **AND** is not the 60s `provider-error` tier used for generic 502 overload

#### Scenario: Structured output does not retry on restriction

- **WHEN** a DeepSeek Web request includes `response_format: json_schema`
- **AND** the upstream returns credential restriction on the final turn
- **THEN** the executor does not perform JSON/schema retry attempts
- **AND** control returns to router failover immediately

### Requirement: Autodefault failover forward after slot restriction

When autodefault encounters credential restriction on a candidate, it SHALL continue
the candidate walk to deliver a response when another eligible path exists.

The walk SHALL:

- Poison only the **restricted credential slot**, not the entire provider name
- Allow **inter-credential** failover (e.g. second DeepSeek session)
- Allow **inter-provider** failover including **stability-band escalation up** on
  another provider when the client intent floor is satisfied
- **NOT** attempt smaller or alternate models on the **same restricted credential**
  expecting a different outcome

#### Scenario: Second DeepSeek session serves after first is muted

- **WHEN** `deepseek-web-default` is credential-restricted
- **AND** `deepseek-web-2` is valid and not restricted
- **THEN** autodefault completes via `deepseek-web-2`
- **AND** route trace records failover from restricted slot

#### Scenario: Stability escalation on another provider after DeepSeek restriction

- **WHEN** all configured DeepSeek Web slots are restricted or absent
- **AND** a Gemini free credential has fast-band models exhausted but stability-band eligible
- **THEN** autodefault MAY succeed via stability-band model on Gemini
- **AND** does not return 403 to the client if a higher band candidate succeeds

#### Scenario: Same restricted slot does not downgrade model

- **WHEN** `deepseek-web-default` is credential-restricted on `deepseek-chat`
- **THEN** autodefault does not attempt `deepseek-reasoner` on the same credential in the same walk

### Requirement: Credential-restriction cooldown catalog tier

Embedded `provider-limits.yaml` SHALL define `cooldown.credential-restriction` in
global defaults and for browser-session free providers (`deepseek-web`, and
fallback alignment with `chatgpt-web` abuse-block semantics).

#### Scenario: deepseek-web defines credential-restriction cooldown

- **WHEN** embedded provider limits are loaded
- **THEN** provider `deepseek-web` defines `cooldown.credential-restriction` of **4 hours**
- **AND** global cooldown defaults define a fallback `credential-restriction` duration

### Requirement: Emulator credential-restricted profile

The upstream provider emulator SHALL support a **`credential-restricted`** force
profile that returns HTTP 403 with `error.code=credential_restricted` and optional
`restricted_until` for deterministic verification.

#### Scenario: Emulator injects restriction without live API

- **WHEN** routing_load targets a candidate with force profile `credential-restricted`
- **THEN** the gateway classifies the failure as credential restriction
- **AND** applies slot cooldown and failover without calling chat.deepseek.com

### Requirement: Automated tests without live restricted accounts

The gateway SHALL ship tests that validate the signal taxonomy, HTTP mapping, router
cooldown/failover, and autodefault forward paths without requiring a live muted
DeepSeek account.

#### Scenario: Unit tests cover adapter event extraction

- **WHEN** CI runs upstream-credential-restriction tests
- **THEN** DeepSeek biz JSON fixtures produce `CredentialRestricted` with parsed `restricted_until`
- **AND** SSE/empty-response misclassification regressions are covered

#### Scenario: routing_load covers slot failover

- **WHEN** routing_load scenario `deepseek_credential_restricted_failover` runs
- **THEN** restricted slot A is attempted once, slot B succeeds
- **AND** metrics/trace include `credential_restricted`

#### Scenario: Four-slot partial mute matrix

- **WHEN** routing_load scenario `deepseek_four_slot_partial_restriction` runs
- **AND** exactly one of four DeepSeek credential slots returns credential restriction
- **THEN** autodefault succeeds via the first healthy slot among the remaining three
- **AND** muted slot `deepseek-web-default` is not retried in the same request walk

#### Scenario: Two of four slots muted

- **WHEN** `deepseek-web-default` and `deepseek-web-2` return credential restriction
- **AND** `deepseek-web-3` and `deepseek-web-4` are healthy
- **THEN** autodefault succeeds via `deepseek-web-3` (first healthy after muted prefix)
- **AND** `deepseek-web-4` is not required when slot 3 succeeds

#### Scenario: Three of four slots muted

- **WHEN** the first three DeepSeek slots return credential restriction
- **AND** `deepseek-web-4` is healthy
- **THEN** autodefault succeeds via `deepseek-web-4`

#### Scenario: All four slots muted

- **WHEN** all four DeepSeek slots return credential restriction in one request walk
- **THEN** the client receives HTTP 403 with `error.code=credential_restricted`
- **AND** restriction on one slot did not prevent attempting other slots in the same walk

#### Scenario: Cooldown duration from restricted_until

- **WHEN** a classified response includes `restricted_until` two hours ahead
- **THEN** router cooldown is approximately two hours (within buffer tolerance)
- **AND** not the 60s provider-error cooldown
