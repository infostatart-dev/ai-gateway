## 1. Decision

- [ ] 1.1 Record **adopt** / **defer** / **partial** with owner and definition of done
- [ ] 1.2 If defer: document trigger (customer demand, SLA) and revisit cadence
- [ ] 1.3 If partial: document reduced contract vs upstream #299

## 2. If adopt

- [ ] 2.1 Provider registry + model ID rules consistent with peer providers
- [ ] 2.2 Document `/nebius/v1/*` OpenAI-compatible HTTP surface
- [ ] 2.3 Document `NEBIUS_API_KEY` (or fork-canonical env name) alongside peer providers
- [ ] 2.4 Unit tests (parsing/routing) + integration with mock stubs
- [ ] 2.5 Mapping policy for fallbacks in shared mapping artifacts
- [ ] 2.6 Draft `openspec/specs/nebius-provider/spec.md`

## 3. Close

- [ ] 3.1 Archive decision change; open implementation change if **adopt**
