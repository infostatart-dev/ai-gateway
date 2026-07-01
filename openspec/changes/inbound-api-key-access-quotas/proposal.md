## Why

The gateway needs a first-party inbound access layer for serving external
clients with API keys and explicit usage limits. The inherited Helicone
control-plane/auth path is documented as legacy in this fork, and current
rate limiting is user-id scoped, request-only, and not suitable as the
authoritative service boundary.

## What Changes

- Introduce a first-party inbound API key registry loaded from an operator
  managed YAML file, independent of upstream provider credentials and Helicone
  Cloud control-plane state.
- Support live reload of the YAML registry with last-good snapshot semantics:
  valid changes take effect without restart; invalid changes keep serving with
  the previous valid registry.
- Add `ClientAccessContext` as the authoritative inbound identity containing
  key id, subject/user/org identity, plan, scopes, and quota policy reference.
- Enforce inbound scopes before routing (`unified-api`, direct provider proxy,
  and named router scopes).
- Enforce per-key request and token quotas over minute, day, and week windows.
- Add quota storage with process-local memory mode and Redis
  persistence/authority using the same product capability, not as a separate
  follow-up change.
- Reserve estimated tokens before dispatch and reconcile against observed
  usage when available, including streaming response completion.
- Keep Helicone/control-plane authentication only as legacy compatibility;
  it is not the source of truth for the new inbound access layer.

## Capabilities

### New Capabilities

- `inbound-api-key-access-quotas`: First-party inbound API key registry, YAML
  live reload, scoped client access, and request/token quota enforcement with
  memory or Redis state.

### Modified Capabilities

_None._

## Impact

- New config for `client-access` registry path, reload behavior, and quota
  state backend.
- New runtime modules for inbound access registry loading, snapshot reload,
  key hashing/lookup, scope checks, and quota admission.
- Middleware stack changes after route classification and before router dispatch.
- Request body buffering for token estimation on quota-protected endpoints,
  with explicit body-size failure behavior.
- Redis integration for quota counters, reservation state, and cross-replica
  enforcement.
- Tests for YAML parsing, invalid reload, key isolation, scope denial,
  request limits, token reservation/reconciliation, and Redis-backed windows.
