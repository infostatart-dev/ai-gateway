## 1. Decision record

- [ ] 1.1 Choose primary direction **A** (keep `RwLock` + single map) or **B** (finer-grained concurrency when contention is confirmed)
- [ ] 1.2 Write rationale (risk vs benefit) usable as the review bar for future PRs

## 2. If direction B

- [ ] 2.1 Document safety invariants: no collect-keys → `await` → fetch without stable handle; snapshot atomic for tick start or per-check stable ownership
- [ ] 2.2 Name entry criteria and metric signals (thresholds owned by service team; do not invent retroactively)
- [ ] 2.3 Draft `openspec/specs/health-monitor-registry/spec.md` with invariants and acceptance scenarios

## 3. If direction A

- [ ] 3.1 Document that container/lock changes are **not planned** until thresholds fire
- [ ] 3.2 Document where operators watch contention signals

## 4. Close change

- [ ] 4.1 Archive when decision is recorded and linked from design.md
- [ ] 4.2 Open implementation change only if **B** is chosen
