# Active changes

Planning home: [docs/planning.md](../docs/planning.md)

```bash
mise run openspec:list
mise exec -- openspec validate --specs --strict
mise exec -- openspec validate --changes --strict
```

| Change | Type | Upstream | Status |
|--------|------|----------|--------|
| [health-monitor-concurrency](health-monitor-concurrency/) | Decision | [#247](https://github.com/Helicone/ai-gateway/pull/247) | pending |
| [nebius-token-factory](nebius-token-factory/) | Decision | [#299](https://github.com/Helicone/ai-gateway/pull/299) | pending |
| [docs-link-hygiene](docs-link-hygiene/) | Implementation | [#297](https://github.com/Helicone/ai-gateway/issues/297) | ready |
| [azure-openai-provider](azure-openai-provider/) | Decision | [#289](https://github.com/Helicone/ai-gateway/issues/289) | pending |
| [openai-responses-agents-sdk](openai-responses-agents-sdk/) | Decision | [#173](https://github.com/Helicone/ai-gateway/issues/173) | pending |

Completed changes live under [archive/](archive/).

**Cursor:** `/opsx:propose`, `/opsx:apply`, `/opsx:archive`
