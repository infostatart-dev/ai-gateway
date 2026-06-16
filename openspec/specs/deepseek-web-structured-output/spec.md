# deepseek-web-structured-output

## Purpose

Strict and non-strict JSON schema / json_object structured output for DeepSeek
Web browser sessions, autodefault eligibility, and shared validation primitives
with ChatGPT Web.

## Requirements

### Requirement: DeepSeek Web accepts OpenAI json_schema response_format

The `deepseek-web` provider SHALL accept client requests with
`response_format.type = json_schema` (including `strict: true`) on
**`deepseek-chat` and `deepseek-reasoner`** for non-streaming chat completions.

The provider SHALL inject schema instructions into the effective system
prompt (there is no native DeepSeek Web structured-output API field).

#### Scenario: Strict schema request on deepseek-chat

- **WHEN** a client sends a non-streaming chat completion to `deepseek-web/deepseek-chat` with `response_format.json_schema.strict: true` and a valid JSON Schema
- **THEN** the gateway forwards a completion whose prompt includes mandatory strict-mode and schema text
- **AND** the gateway returns assistant `content` that is a single JSON object (no markdown fences)

#### Scenario: Strict schema request on deepseek-reasoner

- **WHEN** a client sends a non-streaming chat completion to `deepseek-web/deepseek-reasoner` with strict json_schema
- **THEN** the gateway injects schema instructions and validates assistant `content` as JSON
- **AND** `reasoning_content` from THINK fragments is returned but is not subject to JSON schema validation

#### Scenario: json_schema object without strict flag

- **WHEN** a client sends `response_format.type = json_schema` with `strict` omitted or `false`
- **THEN** the gateway injects schema instructions without the mandatory strict preamble
- **AND** the gateway still validates that assistant content parses as JSON when schema validation is enabled

#### Scenario: json_object response format

- **WHEN** a client sends `response_format.type = json_object`
- **THEN** the gateway injects a JSON-object-only instruction tail
- **AND** the gateway validates that assistant content parses as JSON

### Requirement: Structured output validation and bounded retries

The DeepSeek Web executor SHALL validate the final turn assistant content when
structured JSON output is required. The executor SHALL check JSON parse success
and, when a schema is present, SHALL verify conformance to the requested schema
(including strict additionalProperties rules when strict mode is enabled).

On validation failure the executor SHALL retry the final turn only, up to two
additional attempts with corrective suffixes, before returning an upstream error
to the client.

#### Scenario: Valid JSON matches schema

- **WHEN** the final turn returns assistant content that parses as JSON and satisfies the requested schema
- **THEN** the gateway returns HTTP 200 with a standard `chat.completion` body

#### Scenario: Invalid JSON triggers retry

- **WHEN** the final turn returns prose or markdown-wrapped JSON on the first attempt
- **AND** structured output is required
- **THEN** the executor performs at least one retry final turn with a corrective suffix before responding

#### Scenario: Retries exhausted

- **WHEN** structured output is still invalid after the maximum retry count
- **THEN** the gateway returns HTTP 502 with an error message indicating structured output failure

#### Scenario: Upload turns skip schema validation

- **WHEN** a request required context upload parts before the final turn
- **THEN** intermediate upload turn responses are not schema-validated
- **AND** schema instructions appear only on the final turn prompt

### Requirement: Autodefault and capability integration for JSON schema

The gateway SHALL register a mapper converter for
`OpenAI ChatCompletions → deepseek-web` so router and `/ai` paths do not fail
with `Converter not present`.

The capability catalog SHALL mark **`deepseek-web/deepseek-chat` and
`deepseek-web/deepseek-reasoner`** as `supports_json_schema: true`.

Budget-aware routers SHALL treat `deepseek-web` as eligible for requests with
`json_schema_required: true` when the model capability matches.

Non-streaming autodefault responses from `deepseek-web` SHALL pass the existing
structured-output gate when `json_schema_required` is set.

#### Scenario: Autodefault with only deepseek-web credential

- **WHEN** `deepseek-web-default` is the only configured autodefault provider
- **AND** a client posts to `/router/autodefault/chat/completions` with `response_format.type = json_schema`
- **THEN** the gateway selects `deepseek-web` instead of returning `Provider not found`

#### Scenario: Mapper present for deepseek-web

- **WHEN** a chat completion is routed to `deepseek-web` through the mapper stack
- **THEN** the request is converted without `Converter not present` internal errors

#### Scenario: Structured-output failover

- **WHEN** autodefault selects `deepseek-web` for a json_schema request
- **AND** the structured-output gate rejects the response after retries
- **THEN** the router attempts the next eligible candidate when one exists

### Requirement: Operator structured-output smoke test

The gateway SHALL provide `deepseek probe --structured-output` that sends a
minimal strict json_schema request through the live session and prints pass or
fail. This command SHALL NOT alter runtime capability flags or configuration.

#### Scenario: Operator verifies session and schema path

- **WHEN** an operator runs `deepseek probe --structured-output` with a valid session
- **THEN** the command reports whether the gateway received valid JSON in assistant content
- **AND** exit code is non-zero on failure

#### Scenario: Probe failure does not disable reasoner routing

- **WHEN** an operator probe fails for deepseek-reasoner
- **THEN** the gateway still advertises `supports_json_schema: true` for that model
- **AND** routed requests are attempted with the same retry and failover behavior as chat

### Requirement: Shared structured-output primitives

Schema parsing, instruction building, and response validation logic SHALL live
in a shared crate/module consumed by both `chatgpt-web` and `deepseek-web` so
strict wording and validation rules do not diverge.

#### Scenario: ChatGPT Web regression after extraction

- **WHEN** structured-output helpers are moved to the shared module
- **THEN** existing `chatgpt-web` structured-output unit tests continue to pass without behavior change

### Requirement: Documentation, changelog, and release

The gateway SHALL document DeepSeek Web JSON schema behavior (both models,
reasoner content vs reasoning_content, retries, autodefault) and SHALL ship in
release **`0.3.0-beta.19`**.

The repository root **`CHANGELOG.md`** SHALL contain a **`## [0.3.0-beta.19]`**
section documenting at minimum:

- DeepSeek Web strict JSON schema and json_object support (chat + reasoner)
- DeepSeek Web context chunking (128k budget, 45k upload parts, PoW cache)
- Autodefault mapper fix for deepseek-web
- Context window catalog update from 65536 to 128000 for deepseek-web models

#### Scenario: Changelog entry on release

- **WHEN** beta.19 is released
- **THEN** CHANGELOG.md includes the beta.19 section with the features above

#### Scenario: Operator documentation

- **WHEN** an operator reads `docs/deepseek-web.md`
- **THEN** JSON schema usage, strict mode semantics, and autodefault interaction are described with example curl

#### Scenario: CI coverage without live API

- **WHEN** CI runs provider tests
- **THEN** schema injection, validation, retry planning, mapper registration, and capability flags are covered without live DeepSeek credentials
