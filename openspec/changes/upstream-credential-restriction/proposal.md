## Why

DeepSeek Web returns **HTTP 200 + `biz_code: 5` (`user is muted`)** with an optional
`mute_until` timestamp. Today the executor treats this as empty SSE → **502
`empty response`** → router applies a **60s `provider-error` cooldown** and
retries the same credential in autodefault. That is wrong: the session is valid,
the account is temporarily restricted, and retrying wastes failover budget.

Smoke on 2026-06-18 confirmed the gap deferred in archived
`deepseek-web-provider` design (*“abuse-block cooldown — defer unless smoke shows
retry storms”*). We need a **systemic** model: classify the **event**
(credential restricted) once, then map it to HTTP status, cooldown duration, and
failover policy in separate layers — the same way **429 + Retry-After** is not
the same thing as “rate limited”.

## What Changes

- Introduce a normalized **`UpstreamFailureKind::CredentialRestricted`** event
  (optional `restricted_until`) at the provider adapter boundary.
- **DeepSeek Web** maps `biz_code: 5` / `user is muted` (+ `mute_until`) to that
  event; stop emitting `EmptyResponse` for JSON biz errors with `code: 0`.
- **Dispatcher** maps the event to a stable client HTTP surface (`403` +
  `error.code: credential_restricted`, optional `restricted_until`) — not 502.
- **Router** maps the event to **`ExhaustionScope::Slot`**, cooldown until
  `restricted_until` when present else catalog **`credential-restriction`**
  tier (fallback aligned with `abuse-block`), and **failover without same-slot
  retries** (including structured-output turn retries).
- **Autodefault** continues the candidate walk: blocked slot → next credential
  or **stability / inter-provider escalation** so the client still gets an
  answer when another provider is eligible.
- Add **`credential-restriction`** cooldown to `deepseek-web` (and global
  defaults); emulator **`credential-restricted`** profile for deterministic
  `routing_load` scenarios.
- **`routing_load` four-slot matrix:** `deepseek_four_slot_partial_restriction`
  verifies 1/4, 2/4, 3/4 muted → first healthy slot wins; 4/4 → terminal 403;
  mute on slot N does not poison sibling slots.
- Unit + routing_load tests anchored on **events and scenarios**, not on wire
  JSON field names in router code.

## Capabilities

### New Capabilities

- `upstream-failure-signals`: Normalized upstream failure taxonomy
  (`CredentialRestricted`, etc.), separation of event vs HTTP/cooldown
  implementation, router consumption rules, emulator injection profile, and
  observability labels.

### Modified Capabilities

- `deepseek-web-provider`: Biz-layer error detection and signal emission for
  account restriction; remove empty-response misclassification.
- `autodefault-intent-routing`: Slot restriction must fail over to other
  credentials and stability/inter-provider candidates without downgrading intent
  floor or retrying the restricted slot.

## Impact

- `crates/deepseek-web/` — executor `turn.rs`, error types, biz JSON parser.
- `ai-gateway/src/dispatcher/deepseek_web.rs` — signal → HTTP mapping.
- `ai-gateway/src/router/retry_after/` — classify `credential_restricted`
  responses; cooldown from `restricted_until` or catalog tier.
- `ai-gateway/src/router/budget_aware/failover_loop.rs` — slot scope + no
  structured retry on restriction.
- `ai-gateway/config/embedded/provider-limits.yaml` — `credential-restriction`
  cooldown for `deepseek-web`.
- `ai-gateway/src/emulated/` + `routing_load/scenarios/` — deterministic tests.
- `docs/deepseek-web.md` — operator playbook for mute / restriction.
