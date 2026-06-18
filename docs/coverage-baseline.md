# Rust lib coverage baseline

Measured with `mise run coverage:lib` (`cargo llvm-cov --workspace --all-features --lib`).

## Scope

- **Included:** workspace `#[test]` / `#[tokio::test]` in `src/lib.rs` modules (`--lib`).
- **Excluded:** `ai-gateway/tests/*.rs` integration tests, `main.rs`, CLI binaries. HTTP handler paths are covered there, not in lib %.

## Workspace totals (2026-06-18, post improvement tranche)

| Metric    | Covered | Total  | %      |
|-----------|---------|--------|--------|
| Lines     | 16,644  | 33,900 | 50.90% |
| Regions   | 21,915  | 43,549 | 50.32% |
| Functions | 1,717   | 3,362  | 51.07% |

## Per-crate line coverage (lib)

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
- Target integration coverage in a future change (`--tests` scope)

## Updating this file

1. `mise run coverage:lib`
2. Copy `TOTAL` line from output
3. Raise `coverage:gate` `--fail-under-lines` only when baseline grows deliberately
