## Why

Upstream PR [#299](https://github.com/Helicone/ai-gateway/pull/299) adds **Nebius Token Factory** as a first-class provider (`/nebius/v1/*`, 30+ models, tests, env). This fork has **no Nebius** integration. We need an explicit adopt / defer / partial decision before porting work.

## What Changes

- Record **adopt**, **defer**, or **partial** decision with owner and definition of done.
- If **adopt**: checklist parity with upstream intent (registry, routes, secrets, tests, mappings).
- If **defer/partial**: document what users must not expect until a follow-up change.

## Capabilities

### New Capabilities

- `nebius-provider`: OpenAI-compatible Nebius Token Factory routes, config, auth, and routing (only if **adopt** or agreed **partial** tier).

### Modified Capabilities

- `provider-registry`: Registration and discovery when Nebius is added (if **adopt**).

## Impact

- Embedded providers config, dispatcher factory, router/capability, rate limits, model mappings, tests/mocks.

**Upstream:** [Helicone/ai-gateway#299](https://github.com/Helicone/ai-gateway/pull/299)

**Migrated from:** `.todos/02-change-request.md`
