## ADDED Requirements

### Requirement: Prompt-json schema delivery class

Upstream models with catalog metadata `json-schema-delivery: prompt` SHALL be
treated as json_schema-capable for routing eligibility. The gateway SHALL NOT
forward `response_format` to the upstream API for these models. Instead the
gateway SHALL inject schema instructions into the effective system prompt using
the shared injection builder.

#### Scenario: Ollama Cloud strips response_format and injects schema

- **WHEN** a client sends `response_format.type = json_schema` to `ollama-cloud/gpt-oss:120b`
- **AND** catalog declares `json-schema-delivery: prompt` for that slug
- **THEN** the upstream request omits `response_format`
- **AND** the upstream request includes schema text in the system message

#### Scenario: Native delivery unchanged

- **WHEN** catalog declares `json-schema-delivery: native` for a model
- **THEN** the gateway forwards `response_format` to the upstream API
- **AND** does not strip the field unless the provider converter explicitly requires injection only

---

### Requirement: Reflection retry on prompt-json validation failure

The gateway SHALL perform exactly one reflection retry on the same credential and
model when the first upstream response fails JSON parse or requested schema
validation and `json-schema-delivery: prompt` is active. The reflection turn
SHALL include the assistant's invalid content and a corrective user message
referencing the requested JSON Schema.

#### Scenario: First response invalid prose triggers reflection

- **WHEN** the first upstream response content is markdown prose
- **AND** the request required json_schema validation
- **THEN** the gateway sends one follow-up turn with corrective schema instructions
- **AND** validates the reflection response

#### Scenario: Reflection succeeds

- **WHEN** the reflection response parses as JSON and satisfies the requested schema
- **THEN** the gateway returns HTTP 200 to the client
- **AND** does not record a JSON-validation cooldown for that model

#### Scenario: At most one reflection per client request

- **WHEN** the reflection response also fails validation
- **THEN** the gateway does not send additional reflection turns for that client request
- **AND** the gateway proceeds to JSON-validation cooldown recording

---

### Requirement: Model-level cooldown after prompt-json double failure

The gateway SHALL record a model-level cooldown for `(credential_id, wire_slug)`
for at least 24 hours when both the initial and reflection responses fail
JSON/schema validation for a `json-schema-delivery: prompt` model. The gateway
SHALL store this cooldown in a **dedicated JSON-validation registry** that is
separate from upstream exhaustion / HTTP 404 model cooldown storage.

#### Scenario: Double failure cools model not credential

- **WHEN** initial and reflection responses both fail schema validation on `ollama-cloud/gpt-oss:120b`
- **THEN** `(credential, gpt-oss:120b)` enters JSON-validation cooldown ≥ 24h
- **AND** other models on the same credential remain eligible

#### Scenario: JSON-validation cooldown is independent of 404 cooldown

- **WHEN** `(credential, gpt-oss:120b)` is in JSON-validation cooldown after prompt-json double failure
- **AND** a different slug on the same credential receives HTTP 404 upstream exhaustion
- **THEN** the JSON-validation cooldown entry for `gpt-oss:120b` remains unchanged
- **AND** the 404 exhaustion cooldown for the other slug is recorded separately

#### Scenario: Cooldown skips slug on subsequent json_schema requests

- **WHEN** a json_schema request arrives while `(credential, gpt-oss:120b)` is in JSON-validation cooldown
- **THEN** the router skips that candidate
- **AND** fails over to the next eligible candidate if any remain

---

### Requirement: Shared injection and reflection primitives

The gateway SHALL implement prompt-json behaviors as reusable primitives callable
from any provider converter that declares `json-schema-delivery: prompt`. Provider
converters SHALL NOT duplicate schema instruction text assembly.

#### Scenario: ChatGPT Web and Ollama Cloud share injection builder

- **WHEN** both providers use prompt-json delivery
- **THEN** they invoke the same schema instruction builder
- **AND** produce equivalent strict-mode preamble when `strict: true`
