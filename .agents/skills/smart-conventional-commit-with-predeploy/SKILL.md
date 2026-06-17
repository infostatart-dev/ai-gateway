---
name: smart-conventional-commit-with-predeploy
description: >-
  Agent playbook: scope-aware pre-deploy (rust clippy/tests OR tooling/openspec
  only), stage safely, English Conventional Commit, push fork, watch CI. Use on
  /smart-conventional-commit-with-predeploy or when user asks to commit/push.
disable-model-invocation: true
---

# Smart conventional commit with pre-deploy

**Agent playbook** for this Rust workspace. Pre-deploy is **scope-aware** — do not run full Rust rebuilds for docs/CI/skills commits. Cross-platform release builds happen on **GitHub CI**, not on a local Mac.

## When to apply

- User invokes `/smart-conventional-commit-with-predeploy`.
- User asks to commit, push, or "fix CI then commit".
- Conventional commit message in **English**.

Default push remote: **`fork`** (`infostatart-dev/ai-gateway`).

---

## Phase 1 — Understand the change

Run in parallel from repo root:

```bash
git status
git diff
git diff --staged
git log -3 --oneline
git diff --name-only
git diff --staged --name-only
```

Decide:

- Files in **this** commit only (include related workflow files the user made in other chats — do not exclude without asking).
- **Scope** (see Phase 2): rust vs tooling vs mixed.
- Single logical commit (split if unrelated).

---

## Phase 2 — Pre-deploy (scope-aware, mandatory)

**Run only checks that match the diff.** Never parallel `cargo`. Show full output.

All commands via **[mise](https://mise.jdx.dev/)** / `mise.toml`.

### What local pre-deploy is NOT

| Not local | Why |
|-----------|-----|
| `cargo build --release` | Release binaries built on CI (`release-latest.yml`) for linux/darwin/windows |
| Cross-platform link check | Mac cannot validate Windows/Linux artifacts; CI matrix does |
| Full clean rebuild | Wastes time; `cargo clean` only if user asks |
| Rust checks on docs-only commits | No `*.rs` changed → skip cargo entirely |

**CI is the source of truth** for full compile, link, integration tests, and tri-platform release artifacts. Local gate = **fast sanity on what you touched**.

### Classify the diff

```bash
# Example: list changed paths
git diff --name-only HEAD; git diff --cached --name-only
```

| Scope | When | Run |
|-------|------|-----|
| **tooling** | No `*.rs`, no `Cargo.toml`/`Cargo.lock` in workspace crates | `mise run predeploy:tooling` if `openspec/` touched; else **no cargo** |
| **rust** | Any `*.rs` or crate manifest change | `mise run predeploy:rust` (fmt → clippy → test-lib) |
| **mixed** | Rust + tooling in one commit | `predeploy:rust` + `openspec:validate-changes` if openspec touched |

### Rust gate (`mise run predeploy:rust`)

Matches `.github/workflows/rust-ci.yml` logic — **incremental** compile in existing `target/` is fine:

| Step | Command | Notes |
|------|---------|-------|
| Format | `mise run predeploy:fmt` | Only if `*.rs` changed; nightly rustfmt |
| Clippy | `mise run predeploy:clippy` | `-D warnings`; may compile deps first time |
| Tests | `mise run predeploy:test-lib` | `cargo test --all-features --lib` |

Add `mise run -- cargo test --tests --all-features` only if `ai-gateway/tests/` or integration test code changed.

### Tooling gate

- `openspec/` changed → `mise run openspec:validate-changes`
- Skills, `mise.toml`, `.github/workflows/*.yml`, markdown only → **skip cargo**; push and let CI run Rust jobs

### On failure

1. Read full output.
2. Fix; re-run **only the failed step**.
3. Do not `cargo clean` unless user asks.

---

## Phase 3 — Stage safely

- No `.env`, session JSON, `target/`, credentials.
- Stage all files for this change (including `release-latest.yml` if part of the work).

```bash
git add <paths>
git diff --staged
```

---

## Phase 4 — Conventional commit (English only)

```
type(scope): imperative summary

Why (1–3 sentences). Intent, not file list.
```

Types: `feat`, `fix`, `refactor`, `test`, `docs`, `style`, `build`, `ci`, `chore`.

---

## Phase 5 — Commit (when user asked)

```bash
git commit -m "$(cat <<'EOF'
type(scope): imperative summary

Why this change matters.
EOF
)"
git status
```

---

## Phase 6 — Push and watch CI

```bash
git push fork HEAD
gh run list --repo infostatart-dev/ai-gateway --limit 3
gh run watch <run-id> --repo infostatart-dev/ai-gateway --exit-status
```

On failure: `gh run view <run-id> --repo infostatart-dev/ai-gateway --log-failed` → fix → re-run scoped Phase 2 → push again.

**After push**, CI runs full Rust CI + (on main success) release matrix — that is where compile/link/linux/windows validation happens.

### Releases (fork)

| Workflow | Purpose |
|----------|---------|
| `version-tag.yml` | On `Cargo.toml` version bump → ensures `v*` tag at HEAD, dispatches `release.yml` + `docker.yml` (do **not** push semver tags manually) |
| `release.yml` | Builds/publishes **versioned** binaries (`workflow_dispatch` only) |
| `release-latest.yml` | Rolling **latest** prerelease after green Rust CI on `main` |

To ship a version: bump `[workspace.package].version` in root `Cargo.toml`, update `CHANGELOG.md`, commit, push **main only**. For a one-off tag without a version bump, run **Version tag** via `workflow_dispatch`.

Use `softprops/action-gh-release@v3` (Node 24). Do not pin `@v2` — it triggers Node 20 deprecation warnings on GitHub-hosted runners.

---

## Anti-patterns

| Do not | Do instead |
|--------|------------|
| Run clippy/tests on docs-only commits | Classify diff; skip cargo |
| Local `cargo build --release` before every commit | Trust CI release workflow |
| Expect Mac build to prove Linux/Windows | Watch CI matrix |
| `cargo clean` by default | Incremental builds |
| Parallel `cargo` commands | Sequential |
| Hide output in `tail` | Full output or `tee` |
| Exclude user's workflow files as "unrelated" | Include if user says they're part of the change |
| Russian commit subjects | English only |

---

## Reference

| Item | Location |
|------|----------|
| Rust CI | `.github/workflows/rust-ci.yml` |
| Version tag + dispatch | `.github/workflows/version-tag.yml` |
| Versioned release | `.github/workflows/release.yml` |
| Rolling latest release | `.github/workflows/release-latest.yml` |
| mise tasks | `mise.toml` |
| OpenSpec | `openspec/`, `docs/planning.md` |
