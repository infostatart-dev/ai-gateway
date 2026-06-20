## ADDED Requirements

### Requirement: Quota admission resolves PacingScope from catalog hierarchy

The gateway SHALL evaluate upstream callability using `PacingScope` resolved from the provider
`quota-profile` in `provider-limits.yaml` and the credential catalog:

| `quota-profile` | L1 account key | L2 model dimension |
|-----------------|----------------|-------------------|
| `per-model` | `credential_id` | yes — `CredentialModel { credential, model }` |
| `per-slot` | `credential_id` | no — models inherit account gate |
| `per-session` | session file path | no — browser session gate |

Limit dimensions (RPM, TPM, RPD, TPD, concurrent, min-interval) SHALL resolve from
`catalog_limit_resolve(provider, tier, model)` without hardcoded router durations.

#### Scenario: Gemini per-model admits per slug

- **GIVEN** provider `gemini` has `quota-profile: per-model`
- **WHEN** admission evaluates `gemini-free-3` + `gemini-3-flash-preview`
- **THEN** scope is `CredentialModel { gemini-free-3, gemini-3-flash-preview }`
- **AND** limits resolve from the catalog entry for that slug

#### Scenario: LongCat per-slot inherits account gate

- **GIVEN** provider `longcat` has no `per-model` profile
- **WHEN** admission evaluates `longcat-default` + `LongCat-Flash-Lite`
- **THEN** scope is `Credential(longcat-default)`
- **AND** TPD limits for `LongCat-Flash-Lite` drive the gate on the shared account scope

#### Scenario: DeepSeek Web per-session uses session path

- **GIVEN** provider `deepseek-web` has `quota-profile: per-session`
- **WHEN** admission evaluates `deepseek-web-2`
- **THEN** scope key is the configured session file path, not `(credential, model)`

---

### Requirement: Admission verdict is feasible only when all dimensions clear

The gateway SHALL expose `QuotaAdmission::evaluate(scope, limits, estimated_tokens)` returning
`AdmissionVerdict { feasible, next_wait, next_available_at, blocked_reason }`.

`feasible` SHALL be `true` only when:

1. `PacingGate::peek_next_wait` returns zero
2. Daily headroom (`rpd`/`tpd`) remains for `estimated_tokens`
3. No active model, slot, or session cooldown blocks the scope
4. No active upstream reconcile block (`apply_upstream_reconcile`) blocks the scope

#### Scenario: RPM wait marks infeasible

- **WHEN** `peek_next_wait` returns any duration greater than zero
- **THEN** `feasible` is `false`
- **AND** `blocked_reason` identifies the limiting dimension (`rpm`, `tpm`, or `min_interval`)

#### Scenario: Catalog TPD exhaustion marks infeasible

- **GIVEN** LongCat Flash-Lite catalog `tpd: 50000000` is exhausted for today
- **WHEN** admission evaluates the scope
- **THEN** `feasible` is `false`
- **AND** `blocked_reason` is `tpd` or `rpd` as appropriate

---

### Requirement: Planned hops skip infeasible candidates without HTTP

Route planning and failover walk SHALL treat `feasible == false` as a hard skip: no upstream HTTP,
no provider-stats attempt increment for that scope on the inbound request.

The walk SHALL NOT sleep on planned hops to probe feasibility except for the terminal candidate
when no other feasible hop remains (`max_terminal_wait`).

#### Scenario: Infeasible preview skipped in ladder

- **WHEN** `gemini-3-flash-preview` on `gemini-free-3` is infeasible at plan time
- **AND** `gemini-3.1-flash-lite` on the same account is feasible
- **THEN** the plan includes flash-lite without HTTP on preview
- **AND** provider-stats shows zero preview attempts for that inbound request

---

### Requirement: Hop-time re-admission before each upstream attempt

Before each planned upstream dispatch, the gateway SHALL re-run admission for that candidate. If the
verdict changed to infeasible since plan time, the hop SHALL be skipped without HTTP.

#### Scenario: Re-admit after first hop consumes RPM

- **GIVEN** hop 1 succeeded and consumed RPM budget on an account scope
- **WHEN** hop 2 is evaluated milliseconds later
- **AND** re-admission shows `next_wait > 0`
- **THEN** hop 2 is skipped without HTTP

---

### Requirement: Upstream reconcile aligns local state to response truth

The gateway MUST, after upstream returns a classifiable quota failure (429 RPM, quota exhausted,
reset headers), call `apply_upstream_reconcile(scope, until_instant)` so subsequent admission
reflects upstream truth before the next inbound request.

Reconcile duration SHALL prefer response headers and classifiers; catalog `cooldown-defaults` SHALL
apply only when upstream provides no usable reset instant.

#### Scenario: OpenRouter free-models-per-day reconcile

- **WHEN** upstream 429 body contains `free-models-per-day`
- **AND** `X-RateLimit-Reset` is present
- **THEN** reconcile blocks the `CredentialModel` scope until that instant
- **AND** the next plan excludes that scope without HTTP

---

### Requirement: Repeat upstream 429 on infeasible scope is a gateway violation

The gateway MUST treat upstream HTTP 429 on a scope that was infeasible at the admit instant
immediately before dispatch as a gateway defect. The gateway MUST increment
`gateway_repeat_429_violations_total` and record `repeat_429_violation=true` on the route trace.

#### Scenario: Repeat 429 on reconciled nemotron slug

- **GIVEN** `(openrouter-default, nvidia/nemotron-3-nano-30b-a3b:free)` was infeasible at admit
- **WHEN** dispatch still occurs and upstream returns 429
- **THEN** `repeat_429_violation` is true
- **AND** `gateway_repeat_429_violations_total` increases by 1
