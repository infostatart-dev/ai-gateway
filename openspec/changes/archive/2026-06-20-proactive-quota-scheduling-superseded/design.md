## Context

**0.5.x** introduced `plan_route_chain()` with `QuotaSnapshot`, credential health, work-unit spread,
and intra-slot ladder escalation. Production on eight Gemini free slots still concentrates traffic on
`gemini-free` and `gemini-free-2` with many upstream 429s.

Root causes in current code:

| Gap | Current behaviour | Operator expectation |
|-----|-------------------|-------------------|
| Headroom threshold | `headroom_score = 0` only when `next_wait > max_cooldown_wait` (~3s) | Any `next_wait > 0` ⇒ do not call |
| Walk-time wait | `wait_for_candidate` sleeps up to 3s then probes HTTP | Skip hop, try next candidate |
| Stale snapshot | Captured once at plan start | Re-peek before each hop |
| Post-429 drift | Model cooldown set; pacing counters may lag upstream | Sync pacing «blocked until T» |
| Repeat 429 | Treated as normal failover signal | Gateway bug if pair already blocked |

Existing building blocks to reuse: `PacingGate::peek_next_wait`, `daily_headroom_available`,
`ModelCooldownKey`, `retry_after` reset-header parsing (0.4.2-beta.4), `CredentialHealthRegistry`.

## Goals / Non-Goals

**Goals:**

- **Oracle-first scheduling**: every `(credential, model)` has `callable(now) -> bool` and
  `next_available_at`.
- **Hard skip**: planner and walk never HTTP to pairs with `next_wait > 0` or active quota cooldown.
- **Sync on 429**: upstream exhaustion updates local pacing so oracle reflects reality before next
  request.
- **Repeat-429 detection**: metric + trace when 429 hits an oracle-blocked pair.
- **Testable KPI**: routing_load proves zero repeat 429 on blocked pairs; N parallel units use N
  headroom slots when available.

**Non-Goals:**

- Distributed oracle (Redis) — single-process v1 only.
- Blocking client requests until T (we skip/failover, not queue indefinitely).
- Changing upstream quota truth (Google/OpenRouter remain source of truth on first 429).
- Forcing all 8 keys active when only 2 have secrets configured.

## Decisions

### D1: `QuotaOracle` as thin facade over existing state

**Decision:** Introduce `QuotaOracle` (module `router/quota_oracle/`) that reads pacing gate,
daily window, model/slot cooldown, and returns `OracleVerdict { callable, next_wait, next_available_at, blocked_reason }`.

**Rationale:** Avoid duplicating limit math; snapshot becomes `OracleVerdict` cache at instant T.

**Alternative rejected:** Separate Redis counter per model — deferred; local pacing already exists.

### D2: Zero-wait headroom rule

**Decision:** `headroom_score = 0` when `!daily_ok` OR `next_wait > Duration::ZERO` (strict, not
`max_cooldown_wait`).

**Rationale:** Matches operator model «увидели лимит — не звоним снова до T». Removes 3s sleep-probe
that causes repeat 429.

**Alternative rejected:** Keep soft threshold — caused production 429 noise.

### D3: Remove sleep-probe in planned failover walk

**Decision:** `wait_for_candidate` returns `false` (skip) when `next_wait > 0` for any planned hop
except the **last** candidate in the entire chain (terminal wait allowed once).

**Rationale:** Plan already ordered alternatives; sleeping on hop 2 while hop 3 has headroom wastes
time and quota.

### D4: Hop-time re-peek

**Decision:** Before each upstream attempt in `dispatch`, call `QuotaOracle::peek(candidate)`;
if not callable, skip without `attempt` counter (same as planner exclusion).

**Rationale:** Ladder hop 1 may acquire RPM; hop 2 must see updated wait.

**Alternative rejected:** Rebuild full plan each hop — heavier; re-peek is sufficient for v1.

### D5: Post-429 `apply_upstream_block`

**Decision:** On `FailoverClass::QuotaExhausted` or RPM 429 with resolved `retry_after_secs` /
`reset_at`, call `PacingGate::apply_upstream_block(until)` and model cooldown to the **same** instant.

**Rationale:** Oracle and pacing stay aligned; next plan excludes pair.

**Implementation note:** `apply_upstream_block` sets internal «virtual consumption» or explicit
`blocked_until` on gate — prefer explicit `blocked_until: Option<Instant>` on gate state (minimal
field) over counter manipulation.

### D6: Repeat-429 guard

**Decision:** Before recording failover from 429, if `OracleVerdict.callable == false` for that pair
at `Instant::now()`, emit `repeat_429_violation` (metric + trace field), do **not** extend cooldown
further.

**Rationale:** Surfaces gateway bugs; prevents cooldown inflation loops.

### D7: Ladder construction uses oracle filter

**Decision:** `plan/build.rs` ladder append only includes models where `oracle.callable` at plan
time; walk re-validates at D4.

**Rationale:** Prevents planning known-saturated preview before flash-lite on same slot.

### D8: Observability fields

**Decision:** Extend provider-stats routing row with `next_available_at` (RFC3339 or null),
`blocked_reason` enum (`rpm`, `rpd`, `cooldown`, `circuit`, `upstream_reset`), and aggregate
`repeat_429_violations` on snapshot.

**Rationale:** Operators can see why slots are idle without reading trace logs.

### D9: Test pyramid

**Decision:**

| Layer | What |
|-------|------|
| Unit | `quota_oracle` matrix: zero-wait, RPD=0, cooldown overlap, post-apply block |
| Unit | `budget_aware_snapshot` updated expectations |
| routing_load | `zero_repeat_429.rs`, `parallel_headroom_spread.rs`, `hop_repeek_after_429.rs` |
| Emulator | Profile `repeat-429-guard` — second 429 on blocked pair fails test |

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Stricter skip reduces success rate when pacing is pessimistic | Post-429 sync calibrates; first 429 still allowed |
| Terminal wait on last candidate increases latency | Only when entire pool blocked — acceptable |
| `blocked_until` on gate duplicates model cooldown | Single source: oracle merges both; document precedence |
| False repeat-429 if clock skew vs reset header | Use monotonic `Instant` from parsed reset, not wall clock compare at edge |

## Migration Plan

1. Ship behind no flag — behaviour change is the fix (0.5.5).
2. Deploy notes: expect lower `upstream_attempts` and 429 count; failover % may drop; idle slots with
   headroom should receive traffic when secrets exist.
3. Rollback: revert release; no schema migration.
4. Stage smoke: 8-key profile, 3 parallel work units, verify provider-stats spread + zero repeat 429
   on nemotron after `free-models-per-day`.

## Open Questions

- **Q1:** Should `max_cooldown_wait` config be deprecated or repurposed for terminal-only wait?
  → **Proposed:** Repurpose as `max_terminal_wait` (last hop only); default 60s.
- **Q2:** OpenRouter RPM vs daily — same `apply_upstream_block` path? → **Yes**, via existing
  classifier output `until` instant.
