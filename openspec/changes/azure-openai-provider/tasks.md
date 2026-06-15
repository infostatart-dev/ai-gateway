## 1. Decision

- [ ] 1.1 Record **adopt** / **defer** / **document-only** with owner and definition of done
- [ ] 1.2 If defer: document revisit trigger (customer tier, upstream merge, etc.)

## 2. If adopt

- [ ] 2.1 Define HTTP contract: OpenAI-shaped vs Azure-native paths; `api-version` supply model
- [ ] 2.2 Define auth v1 scope (`api-key` only vs bearer/Entra — explicit tiers)
- [ ] 2.3 Unit URL/header construction tests + mock integration (no live Azure in CI)
- [ ] 2.4 Draft `openspec/specs/azure-openai-provider/spec.md`

## 3. If defer or document-only

- [ ] 3.1 Single canonical statement in README or linked doc
- [ ] 3.2 Close loop for #289-style confusion

## 4. Close

- [ ] 4.1 Archive; open implementation change only if **adopt**
