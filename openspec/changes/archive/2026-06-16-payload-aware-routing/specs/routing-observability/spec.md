## ADDED Requirements

### Requirement: Per-credential failover and cooldown attribution
Router failover and cooldown metrics SHALL carry a `credential` attribute
identifying the upstream account slot, in addition to the existing provider
attribution, so multi-account behavior (e.g. four Gemini free slots) is
distinguishable without log scraping.

#### Scenario: Failover metric distinguishes free slots
- **WHEN** the router fails over from `gemini-free` to `gemini-free-2`
- **THEN** the failover metric records the originating `credential`
- **AND** the value is distinct from a failover originating on `gemini-default`

### Requirement: Quota-metric attribution on rate-limit outcomes
The router SHALL annotate rate-limit, quota, and overload outcome metrics with a
`quota_metric` attribute describing which limit was hit, using one of `rpm`,
`tpm`, `rpd`, `context`, or `overload`.

#### Scenario: Per-minute token cap failure is labeled tpm
- **WHEN** a candidate returns a per-minute token-cap error (e.g. groq 413 TPM)
- **THEN** the metric is annotated with `quota_metric = tpm`

#### Scenario: Daily quota exhaustion is labeled rpd
- **WHEN** a candidate returns a daily quota-exhausted error
- **THEN** the metric is annotated with `quota_metric = rpd`

#### Scenario: Overload is labeled overload
- **WHEN** a candidate returns a `503` overload response
- **THEN** the metric is annotated with `quota_metric = overload`

### Requirement: Per-request routing trace summary
At the end of a router request, the router SHALL emit one structured summary
event capturing at least: number of upstream hops, total wall-clock duration in
milliseconds, the terminal provider and credential, the terminal status, and
counts of candidates skipped pre-flight by payload-aware filtering.

#### Scenario: Summary emitted on success
- **WHEN** a request completes successfully after several failovers
- **THEN** a single summary event reports hop count, duration, terminal provider/credential, and skipped-candidate counts

#### Scenario: Summary emitted on terminal failure
- **WHEN** a request exhausts all candidates without success
- **THEN** a single summary event reports the same fields with the terminal failure status
