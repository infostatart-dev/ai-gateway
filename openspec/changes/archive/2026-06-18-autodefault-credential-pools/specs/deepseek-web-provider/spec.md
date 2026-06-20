## ADDED Requirements

### Requirement: DeepSeek Web multi-session credential pool

The gateway SHALL support up to **two** DeepSeek Web browser sessions in
autodefault via embedded credential slots `deepseek-web-default` and
`deepseek-web-2`. Each slot SHALL:

- Use `provider: deepseek-web`, `tier: free`, `cost-class: free`
- Resolve from `credentials.<id>.session-file` in the secrets file
- Register only when the session file exists and contains a valid `userToken`
- Participate in credential round-robin for the same `(deepseek-web, model)` pool
- Use an isolated pacing gate keyed by session file path

ChatGPT Web SHALL remain a **single** session slot (`chatgpt-web-default` only);
this requirement does not add ChatGPT multi-session support.

#### Scenario: Two sessions register independently

- **WHEN** secrets define `deepseek-web-default.session-file` and `deepseek-web-2.session-file`
- **AND** both session files are valid
- **THEN** autodefault registers two `deepseek-web` candidates per catalog model
- **AND** each candidate uses its own credential slot id

#### Scenario: Missing second session does not block first

- **WHEN** only `deepseek-web-default` has a valid session file
- **THEN** slot `deepseek-web-2` is omitted
- **AND** autodefault behaves as today with a single DeepSeek Web session

#### Scenario: Round-robin alternates DeepSeek sessions

- **WHEN** both DeepSeek Web slots are configured
- **AND** four consecutive requests route to `deepseek-web/deepseek-chat` at the free tier
- **THEN** the first-selected credential alternates between `deepseek-web-default` and `deepseek-web-2`

## MODIFIED Requirements

### Requirement: Conservative pacing for deepseek-web

The gateway SHALL apply **per-session** pacing for `deepseek-web` from embedded
`provider-limits.yaml`. Each configured session slot SHALL have its own pacing
gate; limits apply per session, not globally across all DeepSeek slots.

#### Scenario: Embedded limits use conservative pacing knobs

- **WHEN** embedded `provider-limits.yaml` is loaded
- **THEN** provider `deepseek-web` defines **`rpm: 6`**, **`concurrent: 1`**, and **`min-interval-ms: 10000`**

#### Scenario: Provider available only with a valid session file

- **WHEN** no valid `deepseek-web` session file is configured for a slot
- **THEN** that slot is not registered and is not routed to

#### Scenario: Two sessions allow two concurrent completions

- **WHEN** two valid DeepSeek Web session slots are configured
- **AND** two requests arrive concurrently that route to different DeepSeek slots
- **THEN** each session may have one in-flight completion
- **AND** a third concurrent request to the same session waits on pacing

#### Scenario: Concurrency and interval gating per session

- **WHEN** multiple `deepseek-web` requests arrive for the same session slot
- **THEN** the gateway serializes them according to concurrent and min-interval limits for that session only

### Requirement: Documentation, tests, and release version

The gateway SHALL document DeepSeek Web two-session setup (secrets `session-file`
paths for `deepseek-web-default` and `deepseek-web-2`, login/import per account),
SHALL test two-slot registry, round-robin, and isolated pacing without live API
in CI, and SHALL update `docs/credentials.md` and `dev/secrets.local.example.yaml`.

#### Scenario: Operator configures second session

- **WHEN** an operator runs `deepseek login` twice with different output paths
- **AND** points `deepseek-web-2.session-file` at the second file in secrets
- **THEN** both sessions join autodefault after gateway restart

#### Scenario: CI covers two-session pacing isolation

- **WHEN** tests run for DeepSeek Web multi-session support
- **THEN** pacing gates for `deepseek-web-default` and `deepseek-web-2` are distinct `Arc` instances
- **AND** round-robin selects both slot ids across repeated dispatches
