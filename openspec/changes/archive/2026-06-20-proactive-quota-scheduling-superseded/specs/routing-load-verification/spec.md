## ADDED Requirements

### Requirement: Proactive quota scheduling scenarios in routing load catalog

The routing load verification catalog SHALL include scenarios proving
`quota-oracle-gate` and updated `quota-headroom-scheduling` behaviour:

| File | Proves |
|------|--------|
| `zero_repeat_429.rs` | After first 429 blocks pair until T, no second HTTP 429 on same pair |
| `parallel_headroom_spread.rs` | N concurrent work units with N headroom slots use N distinct first-hop credentials |
| `hop_repeek_after_429.rs` | First hop 429 blocks pair; replan/re-peek routes to sibling without repeat 429 |

Each scenario SHALL assert `repeat_429_violation` is absent (or zero) on route trace
and provider-stats attempt counts match expected HTTP calls.

#### Scenario: zero_repeat_429 catalog entry

- **WHEN** `zero_repeat_429` runs with emulator `free-models-per-day` profile
- **THEN** first inbound request may receive one upstream 429 on the exhausted slug
- **AND** second concurrent request with same slug does not attempt HTTP on blocked pair
- **AND** `gateway_repeat_429_violations_total` remains 0

#### Scenario: parallel_headroom_spread with eight configured keys

- **WHEN** eight `gemini-free*` credentials have headroom and distinct work unit ids
- **AND** three concurrent autodefault requests arrive
- **THEN** first-hop credentials are pairwise distinct when at least three slots have headroom
- **AND** idle headroom slots receive attempts

#### Scenario: hop_repeek_after_429 ladder walk

- **WHEN** first ladder hop returns 429 and blocks preview model
- **AND** second ladder model on same slot has headroom
- **THEN** walk attempts second model without repeat 429 on preview
- **AND** preview attempt count is exactly 1 for the inbound request

---

### Requirement: budget_aware_snapshot unit tests cover strict zero-wait

`ai-gateway/tests/budget_aware_snapshot.rs` SHALL include cases where any
`peek_next_wait > 0` yields `headroom_score == 0.0`.

#### Scenario: 500ms wait scores zero

- **WHEN** pacing gate reports 500ms peek wait
- **THEN** snapshot headroom for that pair is 0.0
