# Agent instructions (ai-gateway)

Rust LLM proxy/router. See [CLAUDE.md](CLAUDE.md) for architecture and dev commands.

## Planning (OpenSpec)

**Planning home:** `openspec/` — see [docs/planning.md](docs/planning.md).

- Active work: `openspec/changes/<change-id>/` (`proposal.md`, `design.md`, `tasks.md`)
- Living specs: `openspec/specs/`
- CLI: `mise exec -- openspec …` (OpenSpec installed via `mise.toml`)

Use `/opsx:propose`, `/opsx:apply`, `/opsx:archive` in Cursor, or the `openspec-*` skills in Codex.

Do **not** use retired paths `.ai/` or `.todos/` — they were migrated to OpenSpec.

## Commits and pre-deploy

Before any commit or push, follow:

**`.agents/skills/smart-conventional-commit-with-predeploy/SKILL.md`**

Pre-deploy is **scope-aware**: no `cargo` for docs/CI/skills-only diffs; rust changes → clippy + tests (incremental). Full compile/link and linux/windows/macOS release artifacts are validated on **GitHub CI**, not locally.
