## ADDED Requirements

### Requirement: OpenRouter intra-slot stability before cross-provider

The gateway SHALL complete OpenRouter intra-slot stability escalation on the current credential
before cross-provider failover for fast-thinking autodefault requests when OpenRouter has
`quota-profile: per-model`, a free ladder with a stability band, and stability models remain
available that support the payload.

#### Scenario: fast-thinking uses gpt-oss stability before groq when nemotron exhausted

- **WHEN** a fast-thinking json_schema request exhausts OpenRouter fast/capacity bands on
  `openrouter-default` including nemotron 429
- **AND** `openai/gpt-oss-120b:free` on the same credential supports json_schema and has quota
- **THEN** the gateway attempts `openai/gpt-oss-120b:free` before a groq fast-thinking candidate

#### Scenario: Client stability preference over cheapest cross-provider hop

- **WHEN** the client intent is fast-thinking with stability expectation
- **AND** OpenRouter stability band models remain on the current credential
- **THEN** the gateway does not switch to a smaller model on another provider solely for cost
