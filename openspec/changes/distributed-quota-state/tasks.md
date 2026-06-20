## 1. Config and wiring (deferred)

- [ ] 1.1 Add `upstream_pacing.store: local | redis` and Redis connection settings
- [ ] 1.2 Factory hook in `PacingRegistry` for distributed backend when `redis` selected

## 2. Redis scope store

- [ ] 2.1 Implement `pacing_scope_key` → Redis key layout and TTL policy
- [ ] 2.2 Atomic acquire / peek against shared counters (RPM, concurrent, daily windows)
- [ ] 2.3 Publish and read `upstream_reconcile_until` per scope

## 3. Admission integration

- [ ] 3.1 `QuotaAdmission` consults distributed store before local-only verdict
- [ ] 3.2 Degrade to local gates when Redis unreachable (metric + log)

## 4. Tests and docs

- [ ] 4.1 Integration tests with Redis testcontainer or embedded mock
- [ ] 4.2 Operator doc: when to enable, overshoot expectations, failure modes
