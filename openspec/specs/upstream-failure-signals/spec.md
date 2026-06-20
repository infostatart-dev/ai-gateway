# upstream-failure-signals

## Purpose

Normalize upstream failure events (rate limit, credential restriction, invalid credential)
at the provider adapter boundary so router policy does not parse provider-specific wire
formats.

## Requirements

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

#### Scenario: Structured output does not retry on restriction

- **WHEN** a DeepSeek Web request includes `response_format: json_schema`
- **AND** the upstream returns credential restriction on the final turn
- **THEN** the executor does not perform JSON/schema retry attempts
- **AND** control returns to router failover immediately

### Requirement: Autodefault failover forward after slot restriction

When autodefault encounters credential restriction on a candidate, it SHALL continue
the candidate walk to deliver a response when another eligible path exists.

The walk SHALL poison only the **restricted credential slot**, allow **inter-credential**
and **inter-provider** failover, and **NOT** attempt alternate models on the **same
restricted credential**.

#### Scenario: Second DeepSeek session serves after first is muted

- **WHEN** `deepseek-web-default` is credential-restricted
- **AND** `deepseek-web-2` is valid and not restricted
- **THEN** autodefault completes via `deepseek-web-2`

#### Scenario: Stability escalation on another provider after DeepSeek restriction

- **WHEN** all configured DeepSeek Web slots are restricted or absent
- **AND** a Gemini free credential has stability-band headroom
- **THEN** autodefault MAY succeed via stability-band model on Gemini

### Requirement: Emulator credential-restricted profile

The upstream provider emulator SHALL support a **`credential-restricted`** force
profile that returns HTTP 403 with `error.code=credential_restricted` and optional
`restricted_until` for deterministic verification.

#### Scenario: Emulator injects restriction without live API

- **WHEN** routing_load targets a candidate with force profile `credential-restricted`
- **THEN** the gateway classifies the failure as credential restriction
- **AND** applies slot cooldown and failover without calling chat.deepseek.com
