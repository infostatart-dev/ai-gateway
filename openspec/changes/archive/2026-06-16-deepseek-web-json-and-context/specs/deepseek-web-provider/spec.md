## MODIFIED Requirements

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
