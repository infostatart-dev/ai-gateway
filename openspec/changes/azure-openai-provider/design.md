## Decision status

**Status:** pending  
**Owner:** (assign)

## Options

| Option | Summary |
|--------|---------|
| **Adopt** | First-class Azure OpenAI with documented YAML/env |
| **Defer** | Document why and revisit trigger |
| **Document-only** | Explicit non-support / workaround in docs |

## Expected end state

- Written decision with definition of done
- If **adopt**: same discoverability bar as other providers; CI green for agreed test slice
- If **defer/document-only**: no reader infers Azure support without explicit limitation

## Notes

- Entra / managed identity and full Azure SKU policy are easy scope creep — call out v1 vs later in decision record.
- Implementation details are out of scope; scope is decision, contract, acceptance.
