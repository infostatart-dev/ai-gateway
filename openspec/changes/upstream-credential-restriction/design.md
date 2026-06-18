## Context

### Observed failure (DeepSeek Web, 2026-06-18)

```
POST /api/v0/chat/completion
HTTP 200  application/json
{ "code": 0,
  "data": { "biz_code": 5, "biz_msg": "user is muted",
            "biz_data": { "is_muted": 1, "mute_until": 1781861651.742 } } }
```

`users/current` succeeds — session token is valid. Completion is blocked at **account**
scope for ~21h.

### Current pipeline (broken)

```
DeepSeek biz JSON (200, code=0)
        │
        ▼  turn.rs only checks top-level code != 0
   treated as SSE body
        │
        ▼  collect_sse → empty content
   Error::EmptyResponse
        │
        ▼  dispatcher → HTTP 502
        │
        ▼  classify_and_cooldown → FailoverClass::Overload
        │                         provider-error 60s
        └── retry storm on same slot ✗
```

### Existing patterns to reuse (not reinvent)

| Concern | Existing module | Pattern |
|---------|-----------------|---------|
| Body phrase classification | `retry_after/abuse.rs` | Wire text → cooldown tier |
| 429 duration | `retry_after/mod.rs` | **Event** rate-limited; **impl** Retry-After header |
| Slot vs model scope | `quota_scope.rs` | `ExhaustionScope::Slot` poisons credential |
| Abuse long cooldown | `chatgpt-web-stabilization` | `abuse-block` tier in catalog |
| Failover walk | `failover_loop.rs` | `failed_credentials` + next candidate |
| Stability escalation | `autodefault-intent-routing` | Same-slot ladder **up**, not down |

ChatGPT abuse-block today classifies **502 bodies** after the fact. DeepSeek mute is
a **first-class JSON biz signal** — but the router must still consume a **normalized
event**, not DeepSeek field names.

## Goals / Non-Goals

**Goals**

- One **upstream failure signal** taxonomy shared by provider adapters and router.
- **`CredentialRestricted`** event with optional `restricted_until` (mute_until is
  **implementation input**, like Retry-After for 429).
- Fail-fast: no same-slot retries, no structured-output retry loop on restriction.
- Autodefault **failover forward** (second DeepSeek session, Gemini stability band,
  paid providers) so clients still get answers when another path exists.
- Catalog **`credential-restriction`** cooldown on free browser-session providers.
- Deterministic tests: unit (signal parse/map) + `routing_load` (emulator profile).

**Non-Goals**

- Rewriting all providers to signals in one PR (ChatGPT abuse can adopt later).
- Distributed restriction state across gateway replicas.
- Operator UI for mute status.
- Calling DeepSeek support APIs.

## Decisions

### D1 — Event vs implementation (three layers)

```
┌──────────────────────────────────────────────────────────────────┐
│ LAYER 1 — EVENT (provider adapter)                               │
│   UpstreamFailureKind::CredentialRestricted { until: Option }  │
│   Parsed from wire ONCE (DeepSeek biz_code, future providers)  │
└────────────────────────────┬─────────────────────────────────────┘
                             │
┌────────────────────────────▼─────────────────────────────────────┐
│ LAYER 2 — CLIENT HTTP (dispatcher)                             │
│   403 + error.code=credential_restricted                         │
│   optional error.restricted_until (RFC3339)                      │
│   Alternative 429+Retry-After rejected: semantically auth-ish    │
└────────────────────────────┬─────────────────────────────────────┘
                             │
┌────────────────────────────▼─────────────────────────────────────┐
│ LAYER 3 — ROUTER POLICY (retry_after + failover)               │
│   FailoverClass::CredentialRestricted (new)                      │
│   ExhaustionScope::Slot                                          │
│   cooldown = until - now OR catalog credential-restriction       │
└──────────────────────────────────────────────────────────────────┘
```

**Rationale:** `mute_until` and HTTP status are **mapping details**. Tests assert
**events and router outcomes**, not “biz_code=5” inside `failover_loop.rs`.

**Rejected:** DeepSeek-only `if biz_code == 5` in router — duplicates adapter logic.

### D2 — Signal transport: response extension + stable HTTP surface

Provider executor returns `Err(Error::CredentialRestricted { until, message })`.
Dispatcher:

1. Sets HTTP **403** and OpenAI-shaped JSON body.
2. Attaches `UpstreamFailureKind` to `DispatchOutcome` / response extensions for
   router (avoid re-parsing JSON in `classify_and_cooldown`).

Router prefers extension when present; falls back to body pattern
`credential_restricted` for emulator and passthrough paths.

**Rejected:** Extension-only without HTTP mapping — breaks direct `/v1` clients.

### D3 — FailoverClass::CredentialRestricted

New enum variant (distinct from `Overload` and `Transient`):

| Class | Same-slot retry | Skip slot | Skip free siblings | Metric label |
|-------|-----------------|-----------|-------------------|--------------|
| Transient (429 RPM) | next model | no | no | rpm |
| Overload (502) | no | sometimes | often | overload |
| **CredentialRestricted** | **no** | **yes** | **no** (only this credential) | **credential_restricted** |

Maps to `ExhaustionScope::Slot` → `failed_credentials.insert(slot)`.

**Rationale:** Mute is not overload; treating as 502 caused 60s loops. Not auth
invalid either — must not use `auth-error` 30m when `mute_until` is 21h away.

### D4 — Cooldown duration

```rust
cooldown = restricted_until
    .map(|t| t - now)
    .unwrap_or(config.credential_restriction + buffer)
```

- Add catalog field `cooldown.credential-restriction` (default **4h**, same order
  of magnitude as `abuse-block`).
- `deepseek-web` override documents observed mute windows.
- Cap minimum at `retry-after-buffer`; no maximum cap (trust upstream until).

**Rationale:** Same pattern as 429 using Retry-After — **until timestamp is
implementation**, event is restriction.

### D5 — DeepSeek biz JSON parser (adapter only)

In `turn.rs` / new `biz_error.rs`:

```
if content-type json AND code==0 AND data.biz_code present AND biz_code != 0
  → map biz_code to UpstreamFailureKind
  biz_code 5 + mute → CredentialRestricted { until from mute_until }
```

Also handle `code != 0` path (existing) via same mapper table.

Stop passing JSON bodies into SSE collector.

**Rejected:** Treat mute as `SessionAuth` — token exchange still works.

### D6 — Structured-output and executor retries

When final turn returns `CredentialRestricted`:

- **No** `MAX_STRUCTURED_RETRIES` loop.
- **No** PoW/completion retry on same session.

Fail immediately to dispatcher → router failover.

### D7 — Autodefault stability / intent (client still gets an answer)

When `deepseek-web-default` is restricted:

```
Attempt 1: deepseek-web-default / deepseek-chat  → CredentialRestricted
           failed_credentials += deepseek-web-default
Attempt 2: deepseek-web-2 / deepseek-chat        → OK (if second account healthy)
   OR
Attempt 2: gemini-free-N / fast band model        → OK
   OR
Attempt N: gemini-free-N / stability band (2.5-pro) → OK  (escalate UP)
```

Rules:

- Restriction poisons **credential slot**, not provider name — do not skip
  `deepseek-web-2` when only `-default` is muted.
- Stability band on **another** provider is allowed and preferred over returning
  403 to client when escalation band satisfies intent floor.
- **Never** downgrade to a smaller model on the **same restricted slot** (no
  reasoner→chat fallback on muted account — both fail).

Aligns with `autodefault-intent-routing`: stability escalates **up** within slot;
restriction forces **slot exit**, then normal priority walk continues.

### D8 — Free provider limits catalog

`provider-limits.yaml`:

```yaml
deepseek-web:
  cooldown:
    credential-restriction: 4h   # fallback when mute_until absent
    abuse-block: 4h              # optional alias period — keep both keys
```

Global `cooldown-defaults.credential-restriction: 2h` for providers without override.

### D9 — Emulator + routing_load

Add emulator force profile `credential-restricted`:

- HTTP 403, body `error.code=credential_restricted`, optional `restricted_until`.
- Scenario `routing_load/scenarios/deepseek_credential_restricted_failover.rs`:
  slot A restricted → slot B or gemini succeeds; assert single attempt on A until
  cooldown; route trace includes `failover_class=credential_restricted`.
- Scenario `routing_load/scenarios/deepseek_four_slot_partial_restriction.rs`:
  four credential ids; matrix 1/4–4/4 muted; sibling slots stay eligible until
  their own restriction event.

### D10 — Observability

- Route trace: `upstream_failure_kind`, `restricted_until` (when known).
- Metrics: `quota_metric=credential_restricted` (via extended `quota_metric_label`).

## Risks / Trade-offs

- **[Risk] Other DeepSeek biz_codes appear** → Mitigation: extensible biz_code
  table in adapter; unknown biz → `Upstream` with logged code; unit tests per code.
- **[Risk] Extension + HTTP drift** → Mitigation: single dispatcher helper builds
  both; round-trip test extension == classify(body).
- **[Risk] ChatGPT abuse still body-only** → Mitigation: non-goal for this change;
  follow-up maps `looks_like_abuse_block` → same event.
- **[Risk] `restricted_until` clock skew** → Mitigation: clamp to minimum
  `credential-restriction` if timestamp in past.

## Migration Plan

1. Ship signal types + DeepSeek parser + dispatcher mapping (no router change) —
   verify 403 instead of 502.
2. Router class + cooldown + failover slot poisoning.
3. Catalog + emulator profile + routing_load scenario.
4. Docs + CHANGELOG entry.

Rollback: revert router classification first (restrictions become 403 but short
cooldown); adapter mapping safe to keep.

## Open Questions

- None blocking — biz_code 5 confirmed in production smoke. Additional DeepSeek
  restriction codes can be added to mapper table as discovered.
