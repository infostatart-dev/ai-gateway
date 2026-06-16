## ADDED Requirements

### Requirement: DeepSeek web provider serves OpenAI-compatible chat completions

The gateway SHALL expose a `deepseek-web` provider whose models are reachable
through the standard OpenAI-compatible chat completions path, serving responses
from chat.deepseek.com's web API for both streaming and non-streaming requests.

#### Scenario: Non-streaming completion

- **WHEN** a client sends a chat completion request for a `deepseek-web` model with `stream:false` and a valid session
- **THEN** the gateway returns a `chat.completion` JSON body with the assistant message content

#### Scenario: Streaming completion

- **WHEN** a client sends a chat completion request for a `deepseek-web` model with `stream:true`
- **THEN** the gateway returns Server-Sent Events as `chat.completion.chunk` objects terminated by `[DONE]`

#### Scenario: Reasoning model emits reasoning content

- **WHEN** a request targets a reasoning model (e.g. `deepseek-reasoner`)
- **THEN** DeepSeek `THINK` fragments are surfaced as OpenAI `reasoning_content` and `ANSWER` fragments as `content`

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

### Requirement: DeepSeek web provider participates in pacing and routing

The `deepseek-web` provider SHALL be discovered from a session-file credential,
be scoped for per-session pacing, and honor configured concurrency, minimum
interval, and cooldown limits for a free single-session account.

#### Scenario: Provider available only with a valid session file

- **WHEN** no valid `deepseek-web` session file is configured
- **THEN** the provider is not registered as an available credential and is not routed to

#### Scenario: Concurrency and interval gating

- **WHEN** multiple `deepseek-web` requests arrive concurrently
- **THEN** the gateway serializes them according to the configured concurrency and minimum-interval limits for the session
