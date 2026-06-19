## MODIFIED Requirements

### Requirement: json_schema-aware candidate ordering

When a request requires `json_schema` structured output, the router SHALL order
eligible candidates so that providers with proven strict-schema reliability are
preferred, while preserving budget-rank as the primary ordering key. Ordering
SHALL NOT promote a candidate that does not advertise json_schema support.

Within the same budget rank and intent tier band, candidates with
`json-schema-delivery: native` SHALL be ordered before candidates with
`json-schema-delivery: prompt`. The default `json_schema_rank` for native delivery
SHALL be lower (better) than for prompt delivery.

#### Scenario: Strict-schema request prefers reliable providers within budget order

- **WHEN** a request sets `response_format.type = json_schema`
- **THEN** json_schema-capable providers are preferred over non-capable peers at the same budget rank
- **AND** budget rank remains the primary sort key

#### Scenario: Non-capable provider is not promoted

- **WHEN** a candidate does not advertise json_schema support
- **THEN** structured-output ordering does not move it ahead of capable candidates

#### Scenario: Native json_schema ranks above prompt-json delivery

- **WHEN** `openrouter/openai/gpt-oss-120b:free` declares `json-schema-delivery: native`
- **AND** `ollama-cloud/gpt-oss:120b` declares `json-schema-delivery: prompt`
- **AND** both are eligible at the same budget rank and intent tier
- **THEN** the native candidate is ordered before the prompt candidate

#### Scenario: Prompt-json remains eligible

- **WHEN** a candidate declares `json-schema-delivery: prompt`
- **THEN** structured-output ordering treats it as json_schema-capable
- **AND** does not exclude it from the candidate pool

## ADDED Requirements

### Requirement: json-schema-delivery metadata drives capability rank

The gateway SHALL derive structured-output capability from catalog
`json-schema-delivery`:

| Delivery | Routing eligible for json_schema | Default json_schema_rank |
|----------|----------------------------------|--------------------------|
| `native` | yes | 1 |
| `prompt` | yes | 2 |
| `none`   | no  | — |

#### Scenario: Prompt delivery sets rank 2

- **WHEN** embedded catalog sets `json-schema-delivery: prompt` for a model
- **THEN** runtime capability marks `supports_json_schema = true`
- **AND** default `json_schema_rank = 2` unless explicitly overridden
