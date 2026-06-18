## ADDED Requirements

### Requirement: Per-model pacing limit resolution

The gateway SHALL resolve pacing limits from the embedded provider limit catalog
for the tuple `(provider, credential tier, upstream model slug)`. For Gemini,
API slugs with `-preview` suffixes SHALL normalize to catalog keys (e.g.
`gemini-3-flash-preview` → `gemini-3-flash`) before lookup.

#### Scenario: Preview slug resolves catalog limits

- **WHEN** pacing is acquired for `gemini` credential tier `free` and model `gemini-3-flash-preview`
- **THEN** limits are read from catalog entry `gemini-3-flash` under `gemini.tiers.free.models`
- **AND** RPM, TPM, and RPD from that entry are applied

#### Scenario: Unknown model fails open on absent catalog entry

- **WHEN** no catalog entry exists for the normalized model slug
- **THEN** pacing does not block dispatch on RPM/TPM/RPD grounds for that dimension

---

### Requirement: Per-model pacing gate scope

The gateway SHALL maintain separate pacing gates per
`(provider, credential scope, normalized upstream model)` for providers that
declare per-model limits in the embedded catalog. Gemini free tier SHALL use
per-model gates; session providers (`chatgpt-web`, `deepseek-web`) SHALL keep
per-session gates without a model dimension.

#### Scenario: Same credential different models use different gates

- **WHEN** two requests use `gemini-free-8` with models `gemini-3-flash-preview` and `gemini-3.1-flash-lite`
- **THEN** each request acquires a distinct pacing gate instance
- **AND** exhausting RPM on 3-flash does not block 3.1-flash-lite on the same credential

#### Scenario: DeepSeek Web gate unchanged

- **WHEN** pacing is acquired for `deepseek-web-default` and `deepseek-web-2`
- **THEN** gates remain keyed by session path only (no model suffix)

---

### Requirement: Multi-dimension proactive reject

The pacing gate SHALL enforce RPM and TPM minute windows and RPD daily counters
when defined in catalog limits. When any enforced dimension is exhausted, the
gateway SHALL reject before an upstream HTTP call and SHALL compute retry-after
aligned to the dimension boundary (minute rollover for RPM/TPM; daily reset hour
for RPD).

#### Scenario: Per-model RPD exhausted blocks only that model

- **WHEN** the RPD counter for `gemini-free-8` × `gemini-3-flash` reaches the catalog limit
- **THEN** the next request targeting that model on that credential is rejected at the pacing gate
- **AND** a request targeting `gemini-3.1-flash-lite` on the same credential may proceed

#### Scenario: TPM minute window per model

- **WHEN** TPM usage for a model in the current minute plus estimated tokens would exceed the catalog TPM
- **THEN** the gate rejects until the minute window rolls over
- **AND** other models on the same credential are unaffected

---

### Requirement: Dispatcher passes model into pacing acquire

The dispatcher SHALL pass the resolved upstream model slug when acquiring pacing
permits so per-model gates receive the correct catalog limits.

#### Scenario: Chat completion dispatch supplies model

- **WHEN** the budget-aware router dispatches to `gemini/gemini-3-flash-preview` on `gemini-free-8`
- **THEN** `acquire_upstream_pacing` receives provider `gemini`, credential `gemini-free-8`, and model `gemini-3-flash-preview`
