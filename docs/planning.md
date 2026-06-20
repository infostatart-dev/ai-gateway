# Planning (OpenSpec)

This repo uses [OpenSpec](https://openspec.dev) for spec-driven planning — replacing the former Task Magic `.ai/` and `.todos/` workflows.

## Layout

```
openspec/
├── specs/              # Living requirements (source of truth after archive)
└── changes/
    └── <change-id>/
        ├── proposal.md # Why and what
        ├── design.md   # Decisions and constraints
        ├── tasks.md    # Checklist
        └── specs/      # Requirement deltas (when applicable)
```

## Commands (via mise)

```bash
mise install                              # node + openspec CLI
mise exec -- openspec list                # active changes
mise exec -- openspec validate --strict   # validate artifacts
mise run openspec:list
mise run openspec:validate
```

## Agent workflow

| Step | Cursor | Codex |
|------|--------|-------|
| New idea | `/opsx:propose "…"` | openspec-propose skill |
| Explore | `/opsx:explore` | openspec-explore skill |
| Implement | `/opsx:apply` | openspec-apply-change skill |
| Verify | `/opsx:verify` | openspec-verify-change skill |
| Sync specs | `/opsx:sync` | openspec-sync-specs skill |
| Archive | `/opsx:archive` | openspec-archive-change skill |

Skills live under `.cursor/skills/openspec-*` and `.codex/skills/openspec-*` (regenerate with `mise exec -- openspec update`).

### Custom workflow profile

This repo uses a **custom** OpenSpec profile (global `~/.config/openspec/config.json`), not the full expanded set:

| Enabled | Skill | Slash (Cursor) |
|---------|-------|----------------|
| yes | `openspec-propose` | `/opsx:propose` |
| yes | `openspec-explore` | `/opsx:explore` |
| yes | `openspec-apply-change` | `/opsx:apply` |
| yes | `openspec-verify-change` | `/opsx:verify` |
| yes | `openspec-sync-specs` | `/opsx:sync` |
| yes | `openspec-archive-change` | `/opsx:archive` |

**Not enabled** (expanded workflows — add via `mise exec -- openspec config profile`, then `openspec update`):

| Skill | Slash | Purpose |
|-------|-------|---------|
| `openspec-new-change` | `/opsx:new` | Scaffold change step-by-step |
| `openspec-continue-change` | `/opsx:continue` | Create next artifact from deps |
| `openspec-ff-change` | `/opsx:ff` | Fast-forward all planning artifacts |
| `openspec-bulk-archive-change` | `/opsx:bulk-archive` | Archive many changes at once |
| `openspec-onboard` | `/opsx:onboard` | Guided tutorial |

`openspec validate` (CLI) checks artifact structure; `/opsx:verify` checks implementation vs specs/tasks.

## Change types

- **Decision changes** — record adopt/defer/direction (most upstream alignment CRs). Archive when decision is written in `design.md`; open a new change for implementation.
- **Implementation changes** — e.g. `docs-link-hygiene`: proposal → specs → apply → archive.

## Commits

Follow `.agents/skills/smart-conventional-commit-with-predeploy/SKILL.md` before push.
