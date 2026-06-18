# Active changes

Planning home: [docs/planning.md](../../docs/planning.md) · Living specs: [openspec/specs/](../specs/) (17 capabilities)

Refresh this index after propose/apply/archive:

```bash
mise exec -- openspec list
mise exec -- openspec validate --specs --strict
mise exec -- openspec validate --changes --strict
```

**Cursor:** `/opsx:propose`, `/opsx:apply`, `/opsx:archive`

## Fork work (autodefault / stage)

| Change | Focus | Tasks | Status |
|--------|-------|-------|--------|
| [routing-load-verification](routing-load-verification/) | Concurrent routing_load + provider-stats assertions | 20/22 | in progress |
| [gateway-load-acceptance](gateway-load-acceptance/) | Stage hardening: payload gate, pacing, failover scope | 13/20 | in progress |
| [per-model-quota-domain](per-model-quota-domain/) | Unified per-model quota domain + OpenRouter consumer | 0/28 | proposed |
| [autodefault-credential-pools](autodefault-credential-pools/) | Mini/nano binding audit, Gemini×16, DeepSeek×2 | 0/20 | proposed |
| [upstream-credential-restriction](upstream-credential-restriction/) | CredentialRestricted signal, DeepSeek mute, slot failover + stability | 0/24 | proposed |

## Upstream / decision backlog

| Change | Type | Upstream | Tasks | Status |
|--------|------|----------|-------|--------|
| [docs-link-hygiene](docs-link-hygiene/) | Implementation | [#297](https://github.com/Helicone/ai-gateway/issues/297) | 0/12 | pending |
| [health-monitor-concurrency](health-monitor-concurrency/) | Decision | [#247](https://github.com/Helicone/ai-gateway/pull/247) | 0/9 | pending |
| [nebius-token-factory](nebius-token-factory/) | Decision | [#299](https://github.com/Helicone/ai-gateway/pull/299) | 0/10 | pending |
| [azure-openai-provider](azure-openai-provider/) | Decision | [#289](https://github.com/Helicone/ai-gateway/issues/289) | 0/9 | pending |
| [openai-responses-agents-sdk](openai-responses-agents-sdk/) | Decision | [#173](https://github.com/Helicone/ai-gateway/issues/173) | 0/11 | pending |

**Active total:** 9 changes — run `openspec list` for live counts.

## Archive

Shipped changes live under [archive/](archive/). After `/opsx:archive`, sync deltas into [openspec/specs/](../specs/).

| Archived change | Shipped capabilities (high level) |
|-----------------|-----------------------------------|
| [2026-06-16-autodefault-routing-priority](archive/2026-06-16-autodefault-routing-priority/) | Cost-class autodefault order |
| [2026-06-16-chatgpt-web-stabilization](archive/2026-06-16-chatgpt-web-stabilization/) | ChatGPT Web pacing + abuse cooldown |
| [2026-06-16-credential-secrets-local](archive/2026-06-16-credential-secrets-local/) | Secrets-file credentials |
| [2026-06-16-deepseek-web-provider](archive/2026-06-16-deepseek-web-provider/) | DeepSeek Web provider v1 |
| [2026-06-16-deepseek-web-json-and-context](archive/2026-06-16-deepseek-web-json-and-context/) | JSON schema + chunking |
| [2026-06-16-gemini-free-multi-account](archive/2026-06-16-gemini-free-multi-account/) | Gemini free multi-slot |
| [2026-06-16-github-models-provider](archive/2026-06-16-github-models-provider/) | GitHub Models provider |
| [2026-06-16-payload-aware-routing](archive/2026-06-16-payload-aware-routing/) | Payload-aware autodefault |
| [2026-06-16-provider-observability-metrics](archive/2026-06-16-provider-observability-metrics/) | provider-stats observability |
| [2026-06-17-curated-free-providers-expansion](archive/2026-06-17-curated-free-providers-expansion/) | Tier-1 free API providers |
| [2026-06-17-upstream-provider-emulator](archive/2026-06-17-upstream-provider-emulator/) | Upstream emulator (49/51 tasks) |
| [2026-06-18-provider-model-reality](archive/2026-06-18-provider-model-reality/) | Gemini catalog verify, per-model scopes, ladder-only walk (`0.4.2-beta.3`) |
| [2026-06-18-rust-code-coverage](archive/2026-06-18-rust-code-coverage/) | `cargo-llvm-cov` baseline, CI warning job, lib coverage +51% |
