## 1. Upstream failure signal core

- [x] 1.1 Add `UpstreamFailureKind` (+ `CredentialRestricted { restricted_until }`) in shared types or `crates/deepseek-web` re-export path used by dispatcher
- [x] 1.2 Add response extension / dispatch metadata to carry `UpstreamFailureKind` from dispatcher to router without re-parsing provider JSON
- [x] 1.3 Add `FailoverClass::CredentialRestricted` and extend `quota_metric_label` with `credential_restricted`

## 2. DeepSeek Web adapter (event layer)

- [x] 2.1 Extract `biz_error.rs` parser: JSON with `code==0` + `data.biz_code`, map `biz_code=5` → `CredentialRestricted` with `mute_until` → `restricted_until`
- [x] 2.2 Wire parser in `executor/turn.rs` before SSE path; remove JSON-as-SSE path for biz errors
- [x] 2.3 Add `Error::CredentialRestricted` in `deepseek-web` errors; stop mapping mute to `EmptyResponse`
- [x] 2.4 Unit tests: fixtures for mute JSON, non-zero code, happy SSE — assert **event kind**, not HTTP status

## 3. Dispatcher HTTP mapping (implementation layer)

- [x] 3.1 Map `CredentialRestricted` → HTTP 403, `error.code=credential_restricted`, optional `error.restricted_until` in `dispatcher/deepseek_web.rs`
- [x] 3.2 Attach `UpstreamFailureKind` to dispatch outcome / response extensions in same helper
- [x] 3.3 Unit tests: dispatcher maps event → 403 body; regression: no 502 empty response on mute fixture

## 4. Executor retry guard

- [x] 4.1 Short-circuit structured-output retry loop in `executor/run.rs` on `CredentialRestricted`
- [x] 4.2 Unit test: restriction on final turn does not increment structured attempt counter

## 5. Router cooldown and failover

- [x] 5.1 Extend `classify_and_cooldown` to detect extension or 403 `credential_restricted` body → `FailoverClass::CredentialRestricted`, `ExhaustionScope::Slot`
- [x] 5.2 Cooldown: `restricted_until - now` when present, else `credential-restriction + retry-after-buffer`; clamp past timestamps to catalog minimum
- [x] 5.3 Ensure `failover_loop` poisons only restricted credential; verify `deepseek-web-2` not skipped when `-default` restricted
- [x] 5.4 Unit tests: cooldown duration from `restricted_until`; slot poison; not `provider-error` 60s

## 6. Catalog — free provider limits

- [x] 6.1 Add `credential-restriction` to global cooldown defaults in `provider-limits.yaml`
- [x] 6.2 Add `deepseek-web.cooldown.credential-restriction: 4h`
- [x] 6.3 Extend `RouterCooldownConfig` parse + validation tests for new tier

## 7. Autodefault stability failover (routing_load)

- [x] 7.1 Emulator: add `credential-restricted` force profile (403 + `credential_restricted` + optional `restricted_until`)
- [x] 7.2 Scenario `deepseek_credential_restricted_failover`: slot A restricted → slot B OK; assert one attempt on A
- [x] 7.3 Scenario `deepseek_restricted_then_gemini_stability`: all DeepSeek slots restricted → Gemini stability band succeeds (intent floor held)
- [x] 7.4 Route trace fields: `upstream_failure_kind`, `restricted_until`, `failover_class=credential_restricted`

## 8. Observability and docs

- [x] 8.1 Record `credential_restricted` in provider dispatch metrics and route trace
- [x] 8.2 Update `docs/deepseek-web.md` — mute symptoms, `credential_restricted`, re-login vs wait for `restricted_until`
- [x] 8.3 CHANGELOG entry under pending release

## 9. Verification gate

- [x] 9.1 `cargo test` — deepseek-web biz parser, dispatcher mapping, retry_after classification
- [x] 9.2 `cargo test --test routing_load` — new scenarios green
- [x] 9.3 Manual smoke: `deepseek probe` on muted session returns 403 `credential_restricted`, not 502 empty

## 10. Four-slot partial-mute matrix (routing_load)

- [x] 10.1 Add `deepseek_slots(count)` test helper for four credential ids
- [x] 10.2 Scenario `deepseek_four_slot_partial_restriction`: 1/4, 2/4, 3/4 muted → first healthy slot; 4/4 → 403
- [x] 10.3 Spec scenarios: slot isolation — mute on slot N must not poison siblings
- [x] 10.4 Register scenario in `tests/routing_load.rs` and validate OpenSpec strict
