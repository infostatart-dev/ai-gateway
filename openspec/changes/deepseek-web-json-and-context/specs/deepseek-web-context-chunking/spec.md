## ADDED Requirements

### Requirement: Token-budget context planning for DeepSeek Web

The `deepseek-web` provider SHALL plan outgoing prompts using the shared
`web-message-budget` chunk planner with a default DeepSeek Web input context
budget of **128_000** tokens (minus reserved output and protocol overhead),
until a live context probe documents a different safe ceiling.

The planner SHALL use DeepSeek-specific upload part sizing of **45_000** estimated
tokens per part (not the ChatGPT 90_000 default).

The planner SHALL NOT silently truncate user content to fit the budget.

#### Scenario: Small payload single turn

- **WHEN** a chat completion request fits within the DeepSeek input token budget
- **THEN** the executor performs exactly one completion turn

#### Scenario: Large dossier splits into upload parts

- **WHEN** user content exceeds the input token budget
- **THEN** the planner produces multiple turns with `[Context part N/M]` headers
- **AND** intermediate turns use the upload-ack system instruction
- **AND** the joined part payloads preserve the original user text (no `truncated` marker)

#### Scenario: History preserved in materialized payload

- **WHEN** a request includes system messages, multi-turn history, and a current user message
- **THEN** the chunk planner materializes system + history + current message into the payload before splitting
- **AND** the final turn prompt includes instructions to answer after all parts are delivered when more than one part exists

### Requirement: Multi-turn execution on one chat session

For chunked requests the DeepSeek Web executor SHALL create **one**
`chat_session_id` per gateway request and SHALL execute all planned turns
against that session before deleting it.

Each turn SHALL perform token exchange once per gateway request (not per turn),
PoW challenge resolution, and a completion POST reusing the same session id.

#### Scenario: Session reused across upload turns

- **WHEN** a chunk plan contains three upload turns and one final turn
- **THEN** the executor uses the same `chat_session_id` for all four completions
- **AND** deletes the session only after the final turn completes or fails

#### Scenario: Upload turn acknowledgment

- **WHEN** an intermediate context-upload turn completes
- **THEN** the executor accepts any non-empty assistant content (including short acknowledgments such as `OK`) and proceeds to the next turn

#### Scenario: Final turn failure aborts session cleanup

- **WHEN** the final turn fails with a non-recoverable upstream error
- **THEN** the executor still attempts session deletion
- **AND** returns the error to the client

### Requirement: Schema instruction placement with chunking

The gateway SHALL append JSON schema instructions to the system prompt on the
final chunk turn only when structured output is requested, matching ChatGPT Web
behavior. Upload turns SHALL omit schema instructions.

#### Scenario: Strict schema on chunked dossier

- **WHEN** a large json_schema request is split into multiple upload parts
- **THEN** upload turns omit schema instructions
- **AND** the final turn system prompt includes the schema block

### Requirement: Prompt mapping from web turns

Each planned `WebTurn` SHALL be converted to DeepSeek's single `prompt` string
by combining system and user text using labeled turn formatting consistent with
existing DeepSeek prompt conventions.

Image markdown in text SHALL be stripped before upload.

#### Scenario: System plus user in final turn

- **WHEN** the final turn includes both system and user segments
- **THEN** the outgoing DeepSeek prompt contains both segments in deterministic order

### Requirement: Live context limit probe

The gateway SHALL provide `deepseek probe --context-limit` that escalates prompt
size against chat.deepseek.com and reports the largest successful single-prompt
completion for operator calibration.

The probe SHALL NOT assume the DeepSeek API 1M context applies to the web
completion endpoint without empirical success.

#### Scenario: Operator calibrates context window

- **WHEN** an operator runs `deepseek probe --context-limit` with a valid session file
- **THEN** the command prints the maximum successful prompt size observed and guidance for catalog tuning

#### Scenario: Embedded catalog uses conservative default

- **WHEN** no operator override exists
- **THEN** chunk planning uses 128_000 tokens as the DeepSeek Web context budget

### Requirement: PoW answer cache for multi-turn uploads

The DeepSeek Web executor SHALL cache proof-of-work responses for up to **45
seconds** within the same gateway request and chat session so upload turns do
not repeat full PoW solving when the upstream challenge remains valid.

#### Scenario: Second upload turn reuses PoW

- **WHEN** a chunked request executes a second turn within the PoW cache TTL
- **AND** the upstream challenge is unchanged
- **THEN** the executor reuses the cached PoW header instead of solving again

#### Scenario: Stale PoW triggers refetch

- **WHEN** a completion fails due to an invalid PoW response
- **THEN** the executor invalidates the cache entry, fetches a new challenge, and retries once

### Requirement: Pacing semantics for multi-turn requests

Each DeepSeek Web completion turn (upload or final) SHALL count as one paced
completion start under existing `deepseek-web` provider limits.

#### Scenario: Pacing applies per turn

- **WHEN** a chunked request executes four turns
- **THEN** the gateway acquires upstream pacing four times for that client request

### Requirement: Observability for chunk execution

The gateway SHALL emit route trace metadata for budget-aware deepseek-web
completions that includes executor turn count and upload part count for operator
debugging.

#### Scenario: Trace includes turn count

- **WHEN** a chunked DeepSeek Web request succeeds via autodefault
- **THEN** route trace logs include turn count greater than one for multi-part requests

### Requirement: Deprecation of naive history window slicing

The executor SHALL NOT rely on `history_window` reverse-slice truncation as the
primary long-context strategy once chunk planning is enabled.

#### Scenario: history_window ignored for planner path

- **WHEN** chunk planning is active for a request
- **THEN** message inclusion is governed by the chunk plan rather than `history_window` numeric slicing

### Requirement: Documentation and tests

The gateway SHALL document DeepSeek Web long-context behavior (128k default
budget, 45k upload parts, probe command, pacing multiplier, PoW cache) and
SHALL test chunk plans and multi-turn executor sequencing without live DeepSeek
credentials in CI.

#### Scenario: CI chunk plan regression

- **WHEN** CI runs `deepseek-web` unit tests with a synthetic 400k-token dossier
- **THEN** the plan produces multiple upload turns and a final turn without truncation markers
