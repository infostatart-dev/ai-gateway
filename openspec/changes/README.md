# Active changes

Planning home: [docs/planning.md](../../docs/planning.md) · Living specs: [openspec/specs/](../specs/) (27 capabilities)

Refresh this index after propose/apply/archive:

```bash
mise exec -- openspec list
mise exec -- openspec validate --specs --strict
mise exec -- openspec validate --changes --strict
```

**Cursor:** `/opsx:propose`, `/opsx:apply`, `/opsx:verify`, `/opsx:archive` — see [docs/planning.md](../../docs/planning.md) for full workflow.

## In progress

| Change | Focus | Tasks | Status |
|--------|-------|-------|--------|
| [routing-load-verification](routing-load-verification/) | Concurrent routing_load + k6/stubr | 20/22 | in progress |
| [per-model-quota-domain](per-model-quota-domain/) | Unified per-model quota + OpenRouter | 29/30 | in progress |
| [gateway-load-acceptance](gateway-load-acceptance/) | Stage hardening: payload gate, pacing | 13/20 | in progress |
| [distributed-quota-state](distributed-quota-state/) | Phase 2 Redis shared pacing (deferred) | 0/9 | proposed |

## Proposed / backlog (not started)

| Change | Focus | Tasks |
|--------|-------|-------|
| [ollama-prompt-json-per-model-quota](ollama-prompt-json-per-model-quota/) | Ollama prompt JSON + per-model quota | 0/27 |
| [docs-link-hygiene](docs-link-hygiene/) | README / doc link fixes ([#297](https://github.com/Helicone/ai-gateway/issues/297)) | 0/12 |
| [health-monitor-concurrency](health-monitor-concurrency/) | Decision ([#247](https://github.com/Helicone/ai-gateway/pull/247)) | 0/9 |
| [nebius-token-factory](nebius-token-factory/) | Decision ([#299](https://github.com/Helicone/ai-gateway/pull/299)) | 0/10 |
| [azure-openai-provider](azure-openai-provider/) | Decision ([#289](https://github.com/Helicone/ai-gateway/issues/289)) | 0/9 |
| [openai-responses-agents-sdk](openai-responses-agents-sdk/) | Decision ([#173](https://github.com/Helicone/ai-gateway/issues/173)) | 0/11 |

**Active total:** 10 changes — run `openspec list` for live counts.

## Archive

Shipped changes live under [archive/](archive/). After `/opsx:archive`, sync deltas into [openspec/specs/](../specs/).

| Archived change | Shipped (high level) |
|-----------------|----------------------|
| [2026-06-20-proactive-quota-scheduling-superseded](archive/2026-06-20-proactive-quota-scheduling-superseded/) | **Superseded** — never implemented; see hierarchical below |
| [2026-06-18-hierarchical-quota-admission](archive/2026-06-18-hierarchical-quota-admission/) | QuotaAdmission, strict admit, quota tree (**0.5.5**) |
| [2026-06-20-routing-ops-hardening](archive/2026-06-20-routing-ops-hardening/) | Default work-unit, routing_health, quota_capacity (**0.5.1**) |
| [2026-06-19-control-plane-deferred](archive/2026-06-19-control-plane-deferred/) | HTTP startup without Helicone CP (**0.5.3**) |
| [2026-06-18-upstream-credential-restriction](archive/2026-06-18-upstream-credential-restriction/) | CredentialRestricted signal, DeepSeek mute (**beta.5**) |
| [2026-06-18-gemini-per-model-quota-ladder](archive/2026-06-18-gemini-per-model-quota-ladder/) | Per-model pacing gates, ladder escalation |
| [2026-06-18-autodefault-credential-pools](archive/2026-06-18-autodefault-credential-pools/) | Gemini×16, DeepSeek pools |
| [2026-06-18-autodefault-intent-routing](archive/2026-06-18-autodefault-intent-routing/) | Intent tiers, structured output routing |
| [2026-06-19-client-context-route-planning](archive/2026-06-19-client-context-route-planning/) | Caller context, route chain planner (**0.5.1**) |
| [2026-06-18-provider-model-reality](archive/2026-06-18-provider-model-reality/) | Gemini catalog, per-model scopes (**beta.3**) |
| [2026-06-18-rust-code-coverage](archive/2026-06-18-rust-code-coverage/) | cargo-llvm-cov baseline |
| [2026-06-17-upstream-provider-emulator](archive/2026-06-17-upstream-provider-emulator/) | Upstream emulator |
| [2026-06-17-curated-free-providers-expansion](archive/2026-06-17-curated-free-providers-expansion/) | Tier-1 free API providers |
| [2026-06-16-provider-observability-metrics](archive/2026-06-16-provider-observability-metrics/) | provider-stats observability |
| [2026-06-16-payload-aware-routing](archive/2026-06-16-payload-aware-routing/) | Payload-aware autodefault |
| [2026-06-16-github-models-provider](archive/2026-06-16-github-models-provider/) | GitHub Models provider |
| [2026-06-16-gemini-free-multi-account](archive/2026-06-16-gemini-free-multi-account/) | Gemini free multi-slot |
| [2026-06-16-deepseek-web-json-and-context](archive/2026-06-16-deepseek-web-json-and-context/) | JSON schema + chunking |
| [2026-06-16-deepseek-web-provider](archive/2026-06-16-deepseek-web-provider/) | DeepSeek Web provider v1 |
| [2026-06-16-credential-secrets-local](archive/2026-06-16-credential-secrets-local/) | Secrets-file credentials |
| [2026-06-16-chatgpt-web-stabilization](archive/2026-06-16-chatgpt-web-stabilization/) | ChatGPT Web pacing |
| [2026-06-16-autodefault-routing-priority](archive/2026-06-16-autodefault-routing-priority/) | Cost-class autodefault order |
