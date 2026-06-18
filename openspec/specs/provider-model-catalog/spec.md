# provider-model-catalog

## Purpose

Separate upstream wire slugs from embedded limit-catalog keys for per-model
providers, record catalog verification metadata, and gate CI against frozen
ListModels fixtures so phantom slugs cannot ship.

## Requirements

### Requirement: Three-layer model identity

Each routable model entry for providers with `quota-profile: per-model` SHALL
declare:

- **`upstream_slug`** — identifier sent to the provider API on dispatch
- **`catalog_key`** — key used in `provider-limits.yaml` for RPM/TPM/RPD lookup
- **`display_name`** (optional) — operator-facing label matching AI Studio or docs

When `upstream_slug` differs from `catalog_key`, limits resolution SHALL use
`catalog_key` while dispatch SHALL use `upstream_slug`.

#### Scenario: Preview flash maps catalog but sends preview upstream

- **WHEN** model entry has `upstream_slug: gemini-3-flash-preview` and `catalog_key: gemini-3-flash`
- **THEN** pacing resolves limits under `gemini-3-flash`
- **AND** the dispatcher sends `gemini-3-flash-preview` to Google

#### Scenario: Stable GA slug uses same key for both layers

- **WHEN** model entry has `upstream_slug: gemini-3.5-flash` and `catalog_key: gemini-3.5-flash`
- **THEN** pacing and dispatch both use `gemini-3.5-flash`

---

### Requirement: Embedded catalog last-verified metadata

Each provider block in the embedded catalog SHALL record `last_verified_at` (ISO
date) and `verify_source` (`official_api` | `official_docs` | `mixed`) indicating
when upstream slugs were last checked against the provider.

#### Scenario: Gemini catalog declares verification date

- **WHEN** embedded Gemini provider config is loaded
- **THEN** `last_verified_at` is present and not older than the release changelog entry for the slug refresh
- **AND** `verify_source` is `official_api` for Gemini free models

---

### Requirement: CI catalog verification gate

The repository SHALL provide a verify task that asserts every `upstream_slug` (or
legacy bare model string) in embedded `providers.yaml` and every slug in
`provider-ladders.yaml` for a provider appears in a frozen ListModels fixture for
that provider.

The verify task SHALL fail with a clear diff when a configured slug is absent
from the fixture.

#### Scenario: Phantom slug fails verify

- **WHEN** `provider-ladders.yaml` contains `gemini-3.5-flash-preview`
- **AND** the frozen Gemini ListModels fixture does not list that slug
- **THEN** the catalog verify task exits non-zero
- **AND** reports the missing slug

#### Scenario: Live slug passes verify

- **WHEN** `upstream_slug: gemini-3.5-flash` is configured
- **AND** the fixture includes `gemini-3.5-flash`
- **THEN** the catalog verify task passes for that entry

---

### Requirement: Slug hygiene for Gemini free tier

The embedded Gemini free-tier routable set SHALL include only slugs verified
against Google Generative Language `ListModels` with `generateContent` support.
The gateway SHALL NOT embed `gemini-3.5-flash-preview` (non-existent stable id).
The gateway SHALL NOT embed `gemini-1.5-*` slugs removed from the free-tier API
surface.

#### Scenario: Corrected 3.5 flash slug

- **WHEN** the Gemini free ladder fast band is loaded
- **THEN** it includes `gemini-3.5-flash`
- **AND** it does not include `gemini-3.5-flash-preview`

#### Scenario: Removed legacy 1.5 models

- **WHEN** embedded `providers.yaml` Gemini models are enumerated
- **THEN** no entry has upstream slug prefix `gemini-1.5-`

---

### Requirement: OpenAI-compat verify pattern documented

The gateway SHALL document how to extend catalog verification to OpenAI-compatible
providers (Groq, GitHub Models, Mistral, OpenRouter) using `GET /v1/models` or
provider-specific public catalog endpoints, using the same fixture + verify task
pattern as Gemini.

#### Scenario: Contributor finds verify extension guide

- **WHEN** a contributor reads `docs/providers.md` catalog verification section
- **THEN** steps to add a new provider fixture and verify task are documented
