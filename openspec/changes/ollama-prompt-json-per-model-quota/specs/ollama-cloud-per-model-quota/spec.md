## ADDED Requirements

### Requirement: Ollama Cloud per-model quota profile

The embedded limit catalog SHALL declare `quota-profile: per-model` for
`ollama-cloud`. Pacing gates, ladder filters, and per-model cooldown maps SHALL
use `(credential_id, wire_slug)` keys for Ollama Cloud the same way as Gemini and
OpenRouter per-model providers.

#### Scenario: Per-model pacing key

- **WHEN** pacing gates a request for `ollama-cloud/gpt-oss:120b` on credential `ollama-cloud-default`
- **THEN** the gate key includes both credential and wire slug
- **AND** usage on `gpt-oss:20b` does not decrement the same counter as `gpt-oss:120b`

---

### Requirement: Subscription 403 locks model not credential

The gateway SHALL classify Ollama Cloud HTTP 403 subscription-or-plan-required
responses as `ExhaustionScope::Model` for the requested wire slug. The gateway
SHALL NOT mark the entire credential or slot as failed when a Pro slug is
attempted on a free-tier key.

#### Scenario: Pro slug 403 on free key

- **WHEN** `ollama-cloud/kimi-k2.6` returns HTTP 403 subscription required on a free key
- **THEN** `ExhaustionScope` is `Model`
- **AND** `gpt-oss:120b` on the same credential remains eligible in the same walk

#### Scenario: Model cooldown after subscription 403

- **WHEN** a Pro slug returns subscription 403
- **THEN** `(credential, pro-slug)` receives long model cooldown
- **AND** the credential health registry does not open a credential circuit

---

### Requirement: Free-tier Ollama Cloud default ladder

The embedded free autodefault ladder for `ollama-cloud` SHALL include at minimum:

- `gpt-oss:120b` with `intent-tier: fast-thinking` and `json-schema-delivery: prompt`
- `gpt-oss:20b` with `intent-tier: fast` and `json-schema-delivery: prompt`

The gateway SHALL rank `gpt-oss:20b` ahead of `gpt-oss:120b` for plain fast intent
requests to reflect lower estimated quota weight.

#### Scenario: Fast intent prefers 20b before 120b

- **WHEN** a fast intent request without json_schema targets ollama-cloud free ladder
- **THEN** `gpt-oss:20b` is ordered before `gpt-oss:120b` at the same budget band

#### Scenario: Fast-thinking intent prefers 120b

- **WHEN** a fast-thinking intent request targets ollama-cloud free ladder
- **THEN** `gpt-oss:120b` is the primary thinking candidate
- **AND** no deprioritized free slugs are listed ahead of it

---

### Requirement: Unverified Ollama slugs removed from embedded catalog

The embedded `ollama-cloud` catalog SHALL NOT ship slugs whose upstream identity or
free-tier behavior is unclear to operators. Removed slugs SHALL be documented in
English in `provider-limits.yaml` notes with the reason (e.g. `glm-4.7`: upstream
identity unclear; removed from embedded catalog).

#### Scenario: glm-4.7 not in embedded catalog

- **WHEN** embedded `providers.yaml` is loaded after this change
- **THEN** `glm-4.7` is absent from the `ollama-cloud` model list
- **AND** limits notes explain removal in English

#### Scenario: Only gpt-oss free slugs in default ladder

- **WHEN** the ollama-cloud free autodefault ladder is loaded
- **THEN** it contains only `gpt-oss:120b` and `gpt-oss:20b`
- **AND** no other free-tier slug appears in the default ladder

---

### Requirement: Weighted quota operator model documented

The embedded `ollama-cloud` limit catalog SHALL document that Ollama Cloud free
usage is a **weighted quota system**, not a raw request counter. Notes SHALL state
that session and weekly percentage meters reflect weighted units where larger models
consume more units per call, with indicative session budget ≈ 300–400 weighted
units per session window.

#### Scenario: Limits notes describe weighted buckets

- **WHEN** an operator reads `provider-limits.yaml` for `ollama-cloud`
- **THEN** notes explain session vs weekly buckets as weighted usage
- **AND** notes list verified free slugs and Pro-only slugs as of last verification date
