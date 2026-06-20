## ADDED Requirements

### Requirement: Provider-stats exposes hierarchical quota tree

`GET /v1/observability/provider-stats` SHALL return quota observability as a tree aligned with
admission hierarchy:

```text
quota[] → { provider, accounts[] } → models[] (when quota-profile: per-model)
```

Each **account** node SHALL include: `credential_id`, `quota_profile`, `calls`, routing health,
`next_available_at`, `blocked_reason`.

Each **model** node (per-model providers only) SHALL include: `slug`, `next_available_at`,
`blocked_reason`, attempt counters when non-zero.

When limits apply only at account level (`per-slot`, `per-session`), model nodes SHALL be omitted
and limits are understood to inherit from the account node.

The flat `providers[]` array SHALL remain for backward-compatible call counters; enriched rows MAY
duplicate `quota_profile`, `next_available_at`, and `blocked_reason` from the tree.

#### Scenario: Gemini account shows per-model children

- **WHEN** provider `gemini` has `quota-profile: per-model`
- **AND** `gemini-free-3` has blocked preview slug and feasible flash-lite slug
- **THEN** account row includes `models[]` with distinct `next_available_at` per slug

#### Scenario: LongCat account has no model children

- **WHEN** provider `longcat` is per-slot
- **THEN** account row has no `models[]`
- **AND** `next_available_at` on the account reflects shared gate state

---

### Requirement: Repeat 429 violations on observability snapshot

The provider-stats snapshot root SHALL include `repeat_429_violations` (count since process start).
The gateway SHALL expose the same counter as OpenTelemetry metric
`gateway_repeat_429_violations_total`. Route trace hops SHALL include `repeat_429_violation` when
applicable.

#### Scenario: Clean deployment shows zero violations

- **WHEN** no infeasible scope receives upstream 429
- **THEN** `repeat_429_violations` is 0
