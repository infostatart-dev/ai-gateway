## Why

Upstream [#297](https://github.com/Helicone/ai-gateway/issues/297) reports broken Quick Start and Introduction links in `README.md`. This fork also has doc drift: stale `helicone-router` repo names, badge URL typos, punctuation in link targets, and placeholder URLs. Operators and contributors need trustworthy onboarding docs.

## What Changes

- Fix README front-door links (Quickstart, Introduction) to canonical `docs.helicone.ai` paths (HTTP 200).
- Normalize repository identity in `CONTRIBUTING.md`, `DEVELOPMENT.md`, and clone/PR instructions → `Helicone/ai-gateway` (or fork canonical remote).
- Audit high-signal external links and badges in root Markdown.
- Optional: adopt a small recurring link-check policy with named owner.

## Capabilities

### New Capabilities

- (none)

### Modified Capabilities

- `documentation-onboarding`: README and contributor doc link accuracy and repo identity.

## Impact

- `README.md`, `CONTRIBUTING.md`, `DEVELOPMENT.md`, root badges; no runtime gateway behavior.

**Upstream:** [Helicone/ai-gateway#297](https://github.com/Helicone/ai-gateway/issues/297)

**Migrated from:** `.todos/03-change-request.md`
