# Change Request: Concurrency model for the health-monitor registry

## Context

- Upstream discussed a PR moving the health-monitor map to **`DashMap`** ([Helicone/ai-gateway#247](https://github.com/Helicone/ai-gateway/pull/247)); review called out a **class of bugs**: a gap between iterating the map and re-acquiring an entry **across `await`** (skipped or misaligned checks).
- This fork uses **`Arc<RwLock<HashMap<...>>>`** for the registry; the PR’s approach is **not** merged. The question is **direction of travel** (whether to change the concurrency model at all), not an immediate implementation mandate.

## What must be done

1. **Record a product/engineering decision on direction** (pick one primary path, or an explicit combination):
   - **A.** Keep **`RwLock` + single map** as the baseline; change only when a measurable bottleneck appears.
   - **B.** Move deliberately toward **finer-grained** concurrency (including a **`DashMap`**-style option or another structure) **if** registry contention is confirmed under target load.
2. **If B is chosen**, separately document **safety invariants** (library-agnostic): no “collect keys → `await` → fetch entry without a stable handle”; the snapshot of work for a tick is **atomic relative to starting checks**, or each check **owns** a stable handle for the async duration.
3. **Entry criterion for B work**: threshold metrics/observations (e.g. latency of `health_monitors` operations, lock wait share, p95/p99 at N routers) — **concrete thresholds are owned by the service team**; this CR does not invent them retroactively.

## Expected end state

- **A written decision: direction A or B** with brief rationale (risk vs benefit), usable as the review bar for future PRs.
- If **A**: explicit wording that container/lock changes are **not planned** until thresholds fire + where to watch signals.
- If **B**: documented **invariants** and **acceptance** for the first iteration (e.g. registering a new monitor is not blocked for the entire duration of all peers’ I/O checks in one tick — if that is a stated goal).

## Notes

- The **direction** of PR #247 (reduce global lock contention) is **reasonable** if the goal is scale (router count / tick frequency); **correctness** is not about the library name but about **honoring invariants** with `async`.
- Current code is **not** equivalent to “we already shipped that PR’s feature another way” — it is **not taking that evolution path** until **B** is chosen.
- Implementation specifics (`DashMap`, shards, `papaya`, etc.) are **out of scope** for this CR; scope is **direction and invariants** only.
