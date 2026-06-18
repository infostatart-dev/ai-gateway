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

### Requirement: DeepSeek web multi-session credential pool

The gateway SHALL support up to **two** DeepSeek Web browser sessions in
autodefault via embedded credential slots `deepseek-web-default` and
`deepseek-web-2`. Each slot SHALL:

- Use `provider: deepseek-web`, `tier: free`, `cost-class: free`
- Resolve from `credentials.<id>.session-file` in the secrets file
- Register only when the session file exists and contains a valid `userToken`
- Participate in credential round-robin for the same `(deepseek-web, model)` pool
- Use an isolated pacing gate keyed by session file path

ChatGPT Web SHALL remain a **single** session slot (`chatgpt-web-default` only).

#### Scenario: Two sessions register independently

- **WHEN** secrets define `deepseek-web-default.session-file` and `deepseek-web-2.session-file`
- **AND** both session files are valid
- **THEN** autodefault registers two `deepseek-web` candidates per catalog model
- **AND** each candidate uses its own credential slot id

#### Scenario: Missing second session does not block first

- **WHEN** only `deepseek-web-default` has a valid session file
- **THEN** slot `deepseek-web-2` is omitted
- **AND** autodefault behaves as today with a single DeepSeek Web session

#### Scenario: Round-robin alternates DeepSeek sessions

- **WHEN** both DeepSeek Web slots are configured
- **AND** four consecutive requests route to `deepseek-web/deepseek-chat` at the free tier
- **THEN** the first-selected credential alternates between `deepseek-web-default` and `deepseek-web-2`

### Requirement: Conservative pacing for deepseek-web

The gateway SHALL apply **per-session** pacing for `deepseek-web` from embedded
`provider-limits.yaml`. Each configured session slot SHALL have its own pacing
gate; limits apply per session, not globally across all DeepSeek slots.

#### Scenario: Embedded limits use conservative pacing knobs

- **WHEN** embedded `provider-limits.yaml` is loaded
- **THEN** provider `deepseek-web` defines **`rpm: 6`**, **`concurrent: 1`**, and **`min-interval-ms: 10000`**

#### Scenario: Provider available only with a valid session file

- **WHEN** no valid `deepseek-web` session file is configured for a slot
- **THEN** that slot is not registered and is not routed to

#### Scenario: Two sessions allow two concurrent completions

- **WHEN** two valid DeepSeek Web session slots are configured
- **AND** two requests arrive concurrently that route to different DeepSeek slots
- **THEN** each session may have one in-flight completion
- **AND** a third concurrent request to the same session waits on pacing

#### Scenario: Concurrency and interval gating per session

- **WHEN** multiple `deepseek-web` requests arrive for the same session slot
- **THEN** the gateway serializes them according to concurrent and min-interval limits for that session only

### Requirement: Documentation, tests, and release version

The gateway SHALL document DeepSeek Web two-session setup (secrets `session-file`
paths for `deepseek-web-default` and `deepseek-web-2`, login/import per account),
SHALL test two-slot registry, round-robin, and isolated pacing without live API
in CI, and SHALL update `docs/credentials.md` and `dev/secrets.local.example.yaml`.

#### Scenario: Operator configures second session

- **WHEN** an operator runs `deepseek login` twice with different output paths
- **AND** points `deepseek-web-2.session-file` at the second file in secrets
- **THEN** both sessions join autodefault after gateway restart

#### Scenario: CI covers two-session pacing isolation

- **WHEN** tests run for DeepSeek Web multi-session support
- **THEN** pacing gates for `deepseek-web-default` and `deepseek-web-2` are distinct `Arc` instances
- **AND** round-robin selects both slot ids across repeated dispatches
