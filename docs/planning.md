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
| Archive | `/opsx:archive` | openspec-archive-change skill |

Skills installed by `openspec init` live under `.cursor/skills/openspec-*` and `.codex/skills/openspec-*`.

## Change types

- **Decision changes** — record adopt/defer/direction (most upstream alignment CRs). Archive when decision is written in `design.md`; open a new change for implementation.
- **Implementation changes** — e.g. `docs-link-hygiene`: proposal → specs → apply → archive.

## Commits

Follow `.agents/skills/smart-conventional-commit-with-predeploy/SKILL.md` before push.
