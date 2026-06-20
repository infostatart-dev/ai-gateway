## MODIFIED Requirements

### Requirement: Best-effort attempt when no candidate fits
When payload-aware filtering would remove every candidate, the router SHALL
retain the largest-effective-window candidate(s) as a best-effort tail so the
request receives at least one honest upstream attempt rather than an internal
provider-not-found error. In intent selection mode, best-effort candidates
SHALL be restricted to those at or above the request `floor_tier`; the router
SHALL NOT select a below-floor candidate for best-effort dispatch.

#### Scenario: Oversized request still gets one honest attempt
- **WHEN** the estimated request exceeds every candidate's effective window
- **THEN** the router still dispatches to the largest-window candidate
- **AND** that candidate satisfies intent floor when intent mode is active

#### Scenario: Best-effort does not downgrade deep intent
- **WHEN** a deep-tier intent request exceeds every deep-tier candidate window
- **AND** fast-tier candidates have larger windows
- **THEN** the router does not select a fast-tier candidate for best-effort
- **AND** returns provider-not-found or attempts only at-or-above-floor candidates
