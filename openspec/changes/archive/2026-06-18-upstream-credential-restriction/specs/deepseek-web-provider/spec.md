## ADDED Requirements

### Requirement: DeepSeek biz-layer error detection

The DeepSeek Web executor SHALL inspect JSON completion responses where
`Content-Type` is `application/json`, including cases where HTTP status is 200 and
top-level `code` is 0 but `data.biz_code` indicates failure.

Known restriction signals SHALL map to gateway upstream failure events without
treating the body as SSE.

#### Scenario: User muted biz_code maps to credential restriction

- **WHEN** completion returns HTTP 200 JSON with `data.biz_code: 5` and `biz_msg` indicating mute
- **AND** `data.biz_data.mute_until` is present
- **THEN** the executor returns `CredentialRestricted` with `restricted_until` parsed from `mute_until`
- **AND** does not return `EmptyResponse`

#### Scenario: Non-zero top-level code still mapped

- **WHEN** completion returns JSON with top-level `code != 0`
- **THEN** the executor maps through the biz error table to an appropriate upstream failure
- **AND** does not pass the JSON body to the SSE collector

## MODIFIED Requirements

### Requirement: DeepSeek web authentication and proof-of-work

The provider SHALL authenticate using a persisted `userToken`, exchange it for a
short-lived access token, and solve the DeepSeek `DeepSeekHashV1` proof-of-work
challenge for each completion request before calling the completion endpoint.

#### Scenario: Access token exchange

- **WHEN** a completion is requested and no unexpired access token is cached
- **THEN** the provider calls `users/current` with the `userToken` and caches the returned access token

#### Scenario: Proof-of-work solved per request

- **WHEN** the provider prepares a completion call
- **THEN** it fetches a PoW challenge, computes the answer by SHA3-256 over `"{salt}_{expire_at}_{nonce}"`, and sends the encoded answer in the `X-Ds-Pow-Response` header

#### Scenario: Expired or invalid token

- **WHEN** DeepSeek responds 401/403 to the token exchange or completion
- **THEN** the gateway returns an authentication error indicating the session is invalid and applies the auth-error cooldown

#### Scenario: Account muted is not invalid session

- **WHEN** token exchange succeeds
- **AND** completion returns account mute (`biz_code: 5` / user is muted)
- **THEN** the gateway returns `credential_restricted` (HTTP 403)
- **AND** does not return `invalid_session`
- **AND** applies credential-restriction slot cooldown with `restricted_until` when provided
