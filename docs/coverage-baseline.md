# Rust lib coverage baseline

Measured with `mise run coverage:lib` (`cargo llvm-cov --workspace --all-features --lib`).

## Scope (one formula everywhere)

All coverage tasks and CI use the **same** command flags:

```bash
cargo llvm-cov --workspace --all-features --lib
```

| Task | Extra flags |
|------|-------------|
| `coverage:lib` | `--summary-only` |
| `coverage:report` | `--lcov --output-path lcov.info` |
| `coverage:gate` | `--summary-only --fail-under-lines 48` |

- **Included:** `#[test]` in each crate's `src/` (`--lib`).
- **Excluded:** `ai-gateway/tests/` integration binaries, `main.rs`, CLI binaries, HTTP handlers without lib tests.
- **No alternate slices** — one workspace percentage; integration paths are out of scope until a future `--tests` change.

## Workspace totals (2026-06-18)

| Metric    | Covered | Total  | %      |
|-----------|---------|--------|--------|
| Lines     | 19,187  | 35,634 | **53.84%** |
| Regions   | 24,480  | 45,572 | 53.72% |
| Functions | 1,905   | 3,543  | 53.77% |

## Per-crate line coverage (lib only)

| Crate                 | Lines % |
|-----------------------|---------|
| ai-gateway            | 48.76%  |
| chatgpt-web           | (re-measure after upload test fix) |
| deepseek-web          | (re-measure) |
| upstream-emulator     | (re-measure) |
| web-structured-output | ~100%   |
| web-message-budget    | ~75%    |
| weighted-balance      | ~18%    |

Re-run `mise run coverage:lib` after the improvement tranche to refresh per-crate numbers.

## Gate floor

`mise run coverage:gate` uses `--fail-under-lines 48` (baseline − ~1% buffer).

## Priority modules (next tranche)

| Module | Lines % (approx) | Why |
|--------|------------------|-----|
| `config/validation.rs` | ~55% | Startup mapping errors |
| `config/rate_limit.rs` | ~15% | Rate-limit config parsing |
| `router/budget_aware/selection.rs` | thin | No-candidate paths |
| `router/budget_aware/dispatch.rs` | thin | Autodefault dispatch glue |
| `router/budget_aware/failure.rs` | thin | Failover classification |

## Not in scope for lib %

- `app/run.rs`, `endpoints/*`, `cli/*_login.rs` — integration / manual paths
- `ai-gateway/tests/*` — run via `cargo test --tests`, not counted in lib coverage

## Updating this file

1. `mise run coverage:lib`
2. Copy `TOTAL` line from output
3. Raise `coverage:gate` `--fail-under-lines` only when baseline grows deliberately
