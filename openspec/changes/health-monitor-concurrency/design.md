## Decision status

**Status:** pending  
**Owner:** (assign)

## Options

| Option | Summary |
|--------|---------|
| **A** | Keep `RwLock` + single map; change only when measurable bottleneck appears |
| **B** | Move toward finer-grained concurrency (`DashMap` or equivalent) if registry contention is confirmed under target load |

## Expected end state

- Written decision **A or B** with brief rationale
- If **A**: explicit wording that lock/container changes are not planned until thresholds fire + where to watch signals
- If **B**: documented invariants and acceptance for first iteration (e.g. registering a monitor is not blocked for entire peer I/O tick duration, if that is a goal)

## Notes

- Direction of #247 (reduce global lock contention) is reasonable at scale; correctness is about **async invariants**, not library name.
- Current fork is **not** equivalent to “already shipped #247 another way” — evolution path **B** is not taken until chosen.
- Implementation specifics (`DashMap`, shards, `papaya`, etc.) are **out of scope** for this change; scope is direction and invariants only.
