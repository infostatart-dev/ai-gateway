# Change Request: OpenAI Responses API and Agents SDK compatibility (upstream question #173)

## Context

- Upstream opened **[Helicone/ai-gateway#173](https://github.com/Helicone/ai-gateway/issues/173)** with the title: *“Does this gateway work with OpenAI Responses API and Agents SDK?”* The issue body is **empty**, so the request is entirely carried by the **title**—a **compatibility / parity** question, not a bug report with reproduction steps.
- **OpenAI Responses API** is a **distinct HTTP surface** from classic **`POST /v1/chat/completions`**: clients and first-party SDKs may call **`/v1/responses`** (and related operations as the API evolves). A gateway that only registers chat-completions-style routes **will return 404 or otherwise fail** those clients unless it explicitly supports or transparently forwards them.
- **OpenAI Agents SDK** (language-specific) orchestrates **multiple** calls against the configured **OpenAI base URL**. In practice, “Agents SDK works through the gateway” means: **every HTTP request the SDK issues** to that base URL must either **match a supported route** or be **forwarded compatibly**; otherwise runs fail mid-agent with transport or contract errors unrelated to model quality.
- **Current fork baseline (technical fact, not a product promise):** the OpenAI endpoint module is structured around **`v1/chat/completions`** and typed chat completion request/response/stream types; there is **no** registered **`v1/responses`** (or generic “all of `v1/*`”) path in that module today. Therefore the question in #173 is **not answered by existing routes** until an explicit decision and implementation (or an explicit “unsupported” contract) exists.

## What must be done

1. **Record a product/engineering decision** on how the gateway relates to **#173**, picking **one** primary posture (combinations allowed only if written as phased scope):
   - **A — Full parity (ambitious):** declare support for **OpenAI Responses API** and **Agents SDK** as **tested compatibility targets**—meaning a **defined minimum SDK + API version pair** and a **closed list of HTTP paths/methods** that must work end-to-end through the gateway to the upstream OpenAI deployment.
   - **B — Bounded surface:** support **Responses API only** (or **Responses + explicit allowlist** of additional `v1/*` paths required by a pinned Agents SDK version), with **everything else** explicitly **out of scope** for that release line.
   - **C — Transparent passthrough (generic):** for the OpenAI upstream profile, implement **generic forwarding** for **`/v1/*`** (or a configurable prefix) with **minimal body mutation**, relying on upstream for semantics; still requires **explicit policy** for streaming, auth header forwarding, size limits, and observability hooks so behavior is **predictable**, not “whatever works.”
   - **D — Document-only / defer:** state **non-support** or **partial support** with **hard boundaries** (what fails first: 404 on `/v1/responses`, which SDK versions, which features)—closing the **expectations gap** that #173 exposes without claiming compatibility.
2. **If A, B, or C is chosen**, produce a **compatibility matrix** (artifact, not code) before implementation lands:
   - **Upstream target:** OpenAI API **base URL**, **auth model** (API key vs other), and **whether streaming** is in scope for Responses.
   - **Paths:** at minimum **`POST /v1/responses`**; list any additional **`/v1/...`** paths required by the chosen **Agents SDK version** smoke test (discovered by running the SDK against a recording proxy or reading SDK release notes—**do not guess** undocumented paths in the CR itself).
   - **Streaming:** SSE vs JSON stream vs chunked semantics—**same observable behavior** as direct OpenAI for the chosen SDK scenario, including **error mid-stream** handling.
   - **Failure modes:** HTTP **404/405** vs **502** vs body parse errors—**document which are acceptable** for “compatible” wording.
3. **Define acceptance tests tied to #173’s intent** (library-agnostic description):
   - **Golden-path test:** a minimal **Agents SDK** scenario (agent + at least one tool or structured step—exact recipe tied to the chosen SDK version) succeeds with **`base_url` = gateway** and the same credentials path as production.
   - **Negative test:** a call to an **explicitly unsupported** path fails with a **documented** status and error shape (so users do not interpret it as random breakage).
   - **Regression guard:** chat completions path **remains** compatible for existing users (unless a **breaking** release is explicitly chosen—then semver / changelog rules apply outside this CR).
4. **Documentation obligation for any non-D outcome:** a single **“OpenAI compatibility”** section stating **supported API slice**, **pinned SDK/API versions used for validation**, and **how to request expansion**—so #173-class questions resolve without opening new duplicate issues.
5. **If D is chosen:** update **README + any doc that implies “OpenAI-compatible proxy”** so readers cannot infer **Responses** or **Agents SDK** support without reading the limitation—**honesty beats silent 404**.

## Expected end state

- **A written decision A/B/C/D** with owner, **rationale**, and **definition of done** referenced against #173 (even if upstream remains open, this fork has a **declared** posture).
- If **A/B/C**: #173’s underlying ask (“does it work?”) can be answered **yes, for …** with a **link to the matrix + test recipe**; unsupported combinations answer **no, because …** without hand-waving.
- If **D**: #173’s ask is answered **no / not today** with **clear boundaries** and, where applicable, **workarounds** (e.g. point chat-only integrations at chat routes only)—**no false implication** from marketing language elsewhere.
- **No silent partial support:** if `/v1/responses` is not implemented, operators must not discover that only after deploying Agents SDK to production.

## Notes

- **Issue #173 has no description**—engineering work must **not** invent user-specific reproduction details; instead, anchor on **SDK + API version pins** chosen by the team for validation.
- **“OpenAI-compatible” in README** is **dangerously broad** once SDKs move beyond chat completions; this CR exists to **narrow or honor** that phrase deliberately.
- **Scope control:** full **OpenAI platform** coverage (Assistants v1/v2, Files, Batch, Fine-tune, etc.) is **not implied** by #173’s title alone—any expansion beyond Responses/Agents smoke path needs a **separate CR** or an explicit **phase 2** bullet in the decision record.
- **Security / abuse surface:** generic `/v1/*` passthrough increases **blast radius** (unexpected methods, huge bodies, new endpoints)—must be paired with **policy** (method allowlist, size caps, optional path denylist) if **C** is chosen.
- **Implementation details** (router macros, dispatcher changes, crate upgrades) are **out of scope** for this CR; the CR only fixes **intent, contract, tests, and documentation boundaries**.
