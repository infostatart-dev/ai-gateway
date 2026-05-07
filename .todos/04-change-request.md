# Change Request: Azure OpenAI as a first-class provider (upstream gap #289)

## Context

- Upstream users report **expectation mismatch**: Helicone docs discuss Azure in a **gateway integration** context, while **this gateway repo** exposes no **Azure OpenAI** provider surface—see [Helicone/ai-gateway#289](https://github.com/Helicone/ai-gateway/issues/289) (“Missing Azure OpenAI Provider”, configuration question).
- **Azure OpenAI is not a drop-in OpenAI base-URL swap**: it uses a **resource host**, **`/openai/deployments/{deployment}/…` paths**, an **`api-version` query parameter**, and commonly **`api-key`** (or Entra-based auth) rather than the public OpenAI URL shape alone. The Rust ecosystem already encodes this pattern (e.g. `async-openai`’s [`AzureConfig`](https://docs.rs/async-openai/latest/async_openai/config/struct.AzureConfig.html)); the gap is **product contract in the gateway**, not language capability.
- This fork’s tree today has **no `azure` provider** in embedded config or `InferenceProvider` variants beyond generic **`Named`** + OpenAI-shaped endpoints elsewhere—so **native Azure parity is absent** until explicitly designed.

## What must be done

1. **Record an explicit adopt / defer / document-only decision** for Azure OpenAI parity with the pain in #289:
   - **Adopt:** add a **first-class** Azure OpenAI provider (or a formally supported profile) with **documented** YAML/env for **resource name**, **deployment id**, **api-version**, and **credential mode** (resource key vs Entra—pick supported tiers explicitly).
   - **Defer:** document **why** (capacity, policy, duplicate of Helicone cloud) and a **revisit trigger** (customer tier, revenue, upstream merge).
   - **Document-only:** if product chooses **not** to implement, **update fork + upstream-facing docs** so “Azure in gateway docs” cannot be read as “works in open-source ai-gateway without extra setup.”
2. **If adopt**, define **non-negotiable acceptance** before coding detail:
   - **HTTP contract**: which routes mirror OpenAI (`/v1/...`) vs Azure-native paths; how **`api-version`** is supplied (fixed, per-router, per-deployment).
   - **Auth**: which modes are in v1 (e.g. `api-key` only vs bearer); reject ambiguous “works sometimes” combinations.
   - **Tests**: at least **unit** URL/header construction + **integration** against a **mock** Azure-shaped server (no live Azure subscription required in CI).
3. **If defer or document-only**, still deliver a **single canonical statement** in README or linked doc: **not supported / supported via X / roadmap link**—closing the loop for #289-style confusion.

## Expected end state

- **Written decision** (adopt / defer / document-only) with owner and **definition of done** tied to the chosen tier.
- If **adopt**: operators can enable Azure OpenAI using the same **discoverability bar** as other first-class providers (config snippet + env table + failure messages); **CI green** for the agreed test slice.
- If **defer** or **document-only**: no reader can infer Azure support from **Helicone gateway marketing/docs** without seeing an **explicit** limitation or pointer to the supported product surface.

## Notes

- **Rust + Azure OpenAI** is a **known integration shape** (deployment + api-version + host); implementation in this repo is **routing + dispatcher + config**, not research.
- **Entra / managed identity** and **every** Azure SKU policy are **easy scope creep**—call out **v1 vs later** explicitly in the decision record.
- Line-by-line implementation and dependency choices are **out of scope** for this CR; scope is **decision, contract, and acceptance**.
