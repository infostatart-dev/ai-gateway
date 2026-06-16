# credential-secrets-local

## Purpose

Load all sensitive gateway values from a single gitignored YAML secrets file
instead of scattered credential and integration environment variables. Policy
fields remain in embedded credential catalog; non-secret settings stay in main
config YAML.

## Requirements

### Requirement: Unified secrets file
The gateway SHALL load all sensitive values from a single YAML secrets file. The file SHALL contain a `credentials` map keyed by credential slot id and an optional `integrations` map for non-provider secrets (Helicone API key, AWS/Bedrock credentials).

#### Scenario: Default local secrets path
- **WHEN** `AI_GATEWAY_SECRETS_FILE` is unset and `./dev/secrets.local.yaml` exists
- **THEN** the gateway loads secrets from that file at startup

#### Scenario: Explicit secrets file path
- **WHEN** `AI_GATEWAY_SECRETS_FILE` points to a readable YAML file
- **THEN** that file is the sole secrets source (aside from embedded policy catalog)

### Requirement: Credential slot fields in secrets file
Each `credentials.<slot-id>` entry SHALL support `api-key`, `api-key-file`, or `session-file`. Policy fields (`tier`, `budget-rank`, `cost-class`, `provider`) SHALL NOT be accepted in the secrets file.

#### Scenario: Provider API key from secrets file
- **WHEN** `credentials.openrouter-default.api-key` is set
- **THEN** slot `openrouter-default` registers with that key

#### Scenario: Browser session from secrets file
- **WHEN** `credentials.chatgpt-web-default.session-file` points to a valid session JSON
- **THEN** the chatgpt-web slot registers without `CHATGPT_BROWSER_CLI`

### Requirement: Integrations block in secrets file
The secrets file SHALL support `integrations.helicone.api-key` and optional `integrations.aws` (`access-key`, `secret-key`, `region`) for Bedrock.

#### Scenario: Helicone key from secrets file
- **WHEN** `integrations.helicone.api-key` is set
- **THEN** `config.helicone.api_key` is populated from the secrets file
- **AND** `HELICONE_CONTROL_PLANE_API_KEY` env is not read

#### Scenario: AWS Bedrock from secrets file
- **WHEN** `integrations.aws` is complete
- **THEN** Bedrock requests use those credentials and region
- **AND** `AWS_ACCESS_KEY`, `AWS_SECRET_KEY`, and `AWS_REGION` env overrides are not read

### Requirement: No legacy credential environment variables
The gateway SHALL NOT resolve provider credentials from `AI_GATEWAY_CREDENTIAL_<ID>`, `{PROVIDER}_API_KEY`, `GEMINI_FREE_TIER_*`, `CLOUDFLARE_*` legacy names, or `*_BROWSER_CLI` session env vars.

#### Scenario: Legacy env is ignored
- **WHEN** only `AI_GATEWAY_CREDENTIAL_OPENROUTER_DEFAULT` is set in the environment
- **AND** the secrets file does not define `openrouter-default`
- **THEN** slot `openrouter-default` is not registered

### Requirement: Non-secret config stays in config YAML
Helicone base URL, features, telemetry level, and routers SHALL be configured via the main config file (`-c` / `config/local.yaml`) or `AI_GATEWAY__*` overrides. They SHALL NOT be required in `.env`.

#### Scenario: Local dev uses config plus secrets
- **WHEN** the operator runs `cargo run -- -c ai-gateway/config/local.yaml`
- **AND** `dev/secrets.local.yaml` contains provider keys
- **THEN** the gateway starts without a `.env` file

### Requirement: Production single-file mount
The gateway SHALL support Kubernetes deployment with one mounted secrets YAML and `AI_GATEWAY_SECRETS_FILE` pointing to the mount path.

#### Scenario: Mounted secrets file in cluster
- **WHEN** `AI_GATEWAY_SECRETS_FILE=/etc/ai-gateway/secrets.yaml` and the file is mounted from a Secret
- **THEN** all credential slots and integrations load from that file

### Requirement: Documentation, example file, and release version
The repository SHALL ship `dev/secrets.local.example.yaml`, SHALL gitignore `dev/secrets.local.yaml`, SHALL document the breaking env removal, and SHALL ship this capability in release **`0.3.0-beta.18`**.

#### Scenario: Contributor verifies secrets loading
- **WHEN** secrets file tests run
- **THEN** discovery, credential fields, integrations.helicone, integrations.aws, and legacy-env ignorance are covered
