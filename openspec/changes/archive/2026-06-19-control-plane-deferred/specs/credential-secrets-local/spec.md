## ADDED Requirements

### Requirement: Helicone features do not gate HTTP bind

Configuring `helicone.features` to any value other than `none` SHALL NOT prevent
the HTTP server from binding in sidecar deployment mode. Helicone API key in the
secrets file remains optional and is only required when Helicone Cloud
observability integration is actively used.

#### Scenario: Features all without Helicone server
- **WHEN** `helicone.features` is `all` in config YAML
- **AND** no control-plane websocket server is running
- **AND** `integrations.helicone.api-key` may or may not be set in secrets
- **THEN** the gateway still starts HTTP on `server.port`
- **AND** credential slots from the secrets file register as today

#### Scenario: Documented local dev path
- **WHEN** documentation describes local development with `config/local.yaml`
- **THEN** it recommends `helicone.features: none`
- **AND** states that Helicone control plane is not part of the dev stack
