## Superseded — do not implement

All tasks cancelled. Shipped as **`hierarchical-quota-admission`** (0.5.5). See
`archive/2026-06-18-hierarchical-quota-admission/` and living spec `quota-admission-control`.

## Historical task list (0/30 — never applied)

## 1. Quota oracle core

- [ ] 1.1 Add `router/quota_oracle/` module with `OracleVerdict` (`callable`, `next_wait`, `next_available_at`, `blocked_reason`)
- [ ] 1.2 Implement `peek(credential, model)` merging pacing peek, daily headroom, model/slot cooldown, upstream block
- [ ] 1.3 Add `PacingGate::apply_upstream_block(until: Instant)` and wire from 429 classification path
- [ ] 1.4 Unit tests: callable matrix (RPM wait, RPD=0, cooldown, upstream block, all clear)
