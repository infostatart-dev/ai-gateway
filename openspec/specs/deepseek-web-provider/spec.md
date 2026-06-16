# deepseek-web-provider

## Purpose

Expose chat.deepseek.com as an OpenAI-compatible `deepseek-web` provider using a
persisted browser `userToken`, proof-of-work per request, conservative
single-session pacing, structured JSON output, and token-budget context chunking.

## Requirements

### Requirement: DeepSeek web provider serves OpenAI-compatible chat completions

The gateway SHALL expose a `deepseek-web` provider whose models are reachable
through the standard OpenAI-compatible chat completions path **and** through
budget-aware routers (including `autodefault`), serving responses from
chat.deepseek.com's web API for both streaming and non-streaming requests.

Structured JSON output and long-context chunking SHALL follow the
`deepseek-web-structured-output` and `deepseek-web-context-chunking`
capabilities.

#### Scenario: Non-streaming completion

- **WHEN** a client sends a chat completion request for a `deepseek-web` model with `stream:false` and a valid session
- **THEN** the gateway returns a `chat.completion` JSON body with the assistant message content

#### Scenario: Streaming completion

- **WHEN** a client sends a chat completion request for a `deepseek-web` model with `stream:true`
- **THEN** the gateway returns Server-Sent Events as `chat.completion.chunk` objects terminated by `[DONE]`

#### Scenario: Reasoning model emits reasoning content

- **WHEN** a request targets a reasoning model (e.g. `deepseek-reasoner`)
- **THEN** DeepSeek `THINK` fragments are surfaced as OpenAI `reasoning_content` and `ANSWER` fragments as `content`

#### Scenario: Autodefault routed completion

- **WHEN** a client sends a chat completion to `/router/autodefault/chat/completions` that resolves to `deepseek-web`
- **THEN** the gateway completes the request without mapper or provider-not-found errors when session and capabilities match

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

### Requirement: Conservative pacing for deepseek-web

The gateway SHALL apply single-session pacing for `deepseek-web` from initial ship.

#### Scenario: Embedded limits use conservative pacing knobs

- **WHEN** embedded `provider-limits.yaml` is loaded
- **THEN** provider `deepseek-web` defines **`rpm: 6`**, **`concurrent: 1`**, and **`min-interval-ms: 10000`**

#### Scenario: Provider available only with a valid session file

- **WHEN** no valid `deepseek-web` session file is configured
- **THEN** the provider is not registered as an available credential and is not routed to

#### Scenario: Concurrency and interval gating

- **WHEN** multiple `deepseek-web` requests arrive concurrently
- **THEN** the gateway serializes them according to the configured concurrency and minimum-interval limits for the session

### Requirement: Documentation, tests, and release version

The gateway SHALL document DeepSeek Web setup (secrets `session-file`, login/import,
probe), JSON schema usage, long-context chunking, autodefault behavior, SHALL
test PoW/SSE/session/dispatcher/chunk/schema behavior without live API in CI,
and SHALL ship cumulative DeepSeek Web enhancements through release
**`0.3.0-beta.19`**.

#### Scenario: Operator obtains a session

- **WHEN** an operator runs `deepseek login` or `deepseek import --token`
- **THEN** a session file with `token` and `saved_at` is written at the configured path

#### Scenario: Operator reads structured output docs

- **WHEN** an operator reads `docs/deepseek-web.md` after beta.19
- **THEN** documentation describes JSON schema and long-context upload behavior
