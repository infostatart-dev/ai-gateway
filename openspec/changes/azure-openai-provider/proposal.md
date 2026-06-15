## Why

Upstream [#289](https://github.com/Helicone/ai-gateway/issues/289) reports missing **Azure OpenAI** as a first-class provider while Helicone docs imply gateway integration. Azure uses resource host, `/openai/deployments/{deployment}/…`, `api-version`, and `api-key`/Entra — not a public OpenAI URL swap. This fork has no native Azure provider today.

## What Changes

- Record **adopt**, **defer**, or **document-only** decision for Azure OpenAI parity.
- If **adopt**: define HTTP contract, auth modes, tests (mock Azure-shaped server).
- If **defer/document-only**: canonical README/doc statement so marketing cannot imply unsupported support.

## Capabilities

### New Capabilities

- `azure-openai-provider`: First-class Azure OpenAI profile (only if **adopt**).

### Modified Capabilities

- `documentation-onboarding`: Explicit Azure support/limitation statements (if **document-only** or **defer**).

## Impact

- Provider config, dispatcher, routing, docs; Rust ecosystem already has Azure patterns (`async-openai` `AzureConfig`).

**Upstream:** [Helicone/ai-gateway#289](https://github.com/Helicone/ai-gateway/issues/289)

**Migrated from:** `.todos/04-change-request.md`
