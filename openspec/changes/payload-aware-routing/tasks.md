## 1. Token estimation

- [x] 1.1 Add a pure-Rust BPE tokenizer dependency and a `token_estimate` module that counts input tokens from a serialized chat request (messages + tools + `response_format.json_schema`).
- [x] 1.2 Resolve `reserved_output` from request `max_tokens`, with a configurable default (proposed 4000); add unit tests for present/absent cases.
- [x] 1.3 Compute the estimate once in `budget_aware/dispatch.rs` (single JSON parse) and populate `RequestRequirements.min_context_tokens = estimated_input + reserved_output`.

## 2. Effective-window filtering

- [x] 2.1 Surface per-model per-minute token caps from `config/provider_limits.rs` via `per_request_token_cap(provider, tier, model)`, read at filter time in `budget_aware/payload.rs`.
- [x] 2.2 Correct/confirm `capability/providers.rs` context windows used for filtering (groq 131072, OpenRouter 131072), keeping budget ranking untouched.
- [x] 2.3 Implement `effective_window = margin(min(context_window, tpm_cap))` in `supports_with_payload` + `payload::filter_payload_capable`; `PayloadBudgetConfig.safety_margin_pct` knob.
- [x] 2.4 Implement fail-open on unknown limits and the largest-window best-effort tail when all candidates are filtered.
- [x] 2.5 Tests: groq TPM skip, OpenRouter context skip, unknown-limit fail-open (`payload.rs`, `capability/tests.rs`).

## 3. Quota-aware Gemini siblings + paid fallback

- [x] 3.1 Extend `router/retry_after` with `FailoverClass` + `classify_and_cooldown` (transient RPM vs RESOURCE_EXHAUSTED daily; 503/502 as overload).
- [x] 3.2 In `budget_aware/failover_loop.rs`, on daily-quota/overload mark remaining same-provider free siblings (same budget rank) as skipped; keep sibling failover for transient RPM.
- [x] 3.3 Guarantee one `gemini-default` attempt after free slots are exhausted/skipped: paid slot has a higher budget rank so sibling-skip never removes it and budget ordering reaches it next.
- [x] 3.4 Tests: RPM sibling failover preserved; daily-quota and 503 skip siblings → paid (`credential_failover.rs`).

## 4. Structured-output ordering

- [x] 4.1 Add `json_schema_rank` on `ModelCapability` (set in `providers.rs`) as a secondary rank key when `json_schema_required` (budget-rank stays primary).
- [x] 4.2 Non-capable providers still filtered by `capability_supports`; response-schema validation/failover unchanged.
- [x] 4.3 Tests: OpenRouter demoted vs groq at equal budget rank (`providers.rs`).

## 5. Observability

- [x] 5.1 Add `credential` attribute to failover/cooldown metrics via `FailoverEvent` / `CooldownEvent`.
- [x] 5.2 Add `quota_metric` attribute (`rpm|tpm|rpd|overload`) via `quota_metric_label` / `quota_metric_from_status`.
- [x] 5.3 Emit one per-request routing trace summary (`budget-aware route summary`) at the end of `run_failover_candidates` (`trace.rs`).
- [x] 5.4 Tests: quota_metric labels + FailoverEvent credential (`metrics/router/tests.rs`).

## 6. Docs, version, validation

- [x] 6.1 Document payload-aware routing, quota-aware Gemini behavior, and new metrics in `docs/providers.md` and `docs/credentials.md`.
- [x] 6.2 Bump workspace version to `0.3.0-beta.16`.
- [x] 6.3 Run `cargo clippy` + targeted tests; `mise exec -- openspec validate payload-aware-routing --strict`.
