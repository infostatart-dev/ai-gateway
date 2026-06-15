## ADDED Requirements

### Requirement: README onboarding links resolve

Root `README.md` Quickstart and Introduction links (and sibling first-hour navigation in the same blocks) SHALL resolve to live Helicone documentation without HTTP 404 for a cold reader.

#### Scenario: New contributor opens README

- **WHEN** a user follows Quickstart or Introduction from `README.md`
- **THEN** the target URL returns HTTP 200
- **AND** the page matches the intended onboarding topic

### Requirement: Consistent repository identity in contributor docs

Contributor documentation SHALL reference `Helicone/ai-gateway` (or this fork's canonical remote) for clone, fork, and PR instructions — not legacy `helicone-router` names.

#### Scenario: Contributor follows DEVELOPMENT clone steps

- **WHEN** a user runs clone instructions from `CONTRIBUTING.md` or `DEVELOPMENT.md`
- **THEN** the repository URL matches the canonical project identity
- **AND** no stale compare/fork URLs remain unless listed in a documented EXCEPTIONS section

### Requirement: High-signal external links are valid or explicitly excepted

Root-level Markdown badges and prose links SHALL not contain obvious typos, truncated URLs, or placeholder fragments presented as live links.

#### Scenario: Maintainer audits root docs

- **WHEN** the agreed file set is reviewed before archive
- **THEN** each external link is valid OR listed in EXCEPTIONS with rationale
- **AND** lockfiles are excluded unless a separate policy adopts automated checking for them
