# Change Request: Nebius Token Factory provider integration (upstream alignment)

## Context

- Upstream proposes **Nebius Token Factory** as a first-class inference provider: OpenAI-compatible **`/nebius/v1/*`** routes, **30+** models, embedded provider config, model-ID parsing (including slash form), router wiring, monitoring/rate-limit alignment, model-mapping fallbacks, **`NEBIUS_API_KEY`**, Docker/env templates, and **unit + integration** tests with mock stubs ([Helicone/ai-gateway#299](https://github.com/Helicone/ai-gateway/pull/299)).
- This fork currently has **no Nebius** references in the tree (no integration ported). The question is whether to **adopt** that upstream capability **as-is in spirit** (same user-visible contract and config surface) or **defer / reject** with recorded rationale.

## What must be done

1. **Record an explicit adopt / defer / partial decision** for Nebius parity with upstream PR #299:
   - **Adopt:** port or re-implement following **existing provider invariants** in this fork (registration, dispatcher factory, capability/routing, health/rate-limit hooks where applicable).
   - **Defer:** document **trigger** (e.g. customer demand, Nebius SLA) and **revisit cadence**.
   - **Partial:** e.g. config-only or mapping-only without direct routes — only if product agrees on **reduced contract** vs upstream.
2. **If adopt**, scope must be **checklist-complete** relative to the PR’s intent (not necessarily file-for-file if the fork’s layout differs):
   - Provider registry + **model ID** rules consistent with other providers.
   - **HTTP surface**: documented **`/nebius/v1/*`** behavior matching OpenAI-compatible expectations.
   - **Secrets**: `NEBIUS_API_KEY` (or fork’s canonical env naming) documented in the same places as peer providers.
   - **Tests**: parsing/routing unit tests + **proxy integration** path with mocks/stubs analogous to upstream.
   - **Mappings**: agreed policy for **fallback entries** in shared mapping artifacts (avoid unbounded or conflicting aliases).
3. **Non-goals** unless explicitly added to this CR: changing unrelated providers, breaking existing routes, or shipping without **parity test bar** for the chosen adopt tier.

## Expected end state

- **Written decision** (adopt / defer / partial) with owner and **definition of done** tied to the chosen tier.
- If **adopt**: operators can enable Nebius using the fork’s standard config/env pattern; **CI green** for the new test targets; **no undocumented** auth header or base-URL behavior vs the agreed contract.
- If **defer** or **partial**: a single place (this CR or linked doc) states **what users must not expect** until a follow-up CR.

## Notes

- PR #299 positions the work as **following existing Helicone provider patterns** and claims **no breaking changes** to the rest of the gateway; any port must **re-validate** that against **this fork’s** diff from upstream.
- **Direction** of the PR (add a major cloud inference provider with full routing and tests) is **standard** for this class of repo; **correctness** is in **consistent registration**, **auth**, and **test coverage**, not in copying filenames blindly.
- Implementation order inside an “adopt” execution is **out of scope** for this CR; this file only fixes **whether** and **what bar** applies.
