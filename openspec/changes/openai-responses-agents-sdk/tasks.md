## 1. Decision

- [ ] 1.1 Choose **A** (full parity), **B** (bounded), **C** (passthrough), or **D** (document-only/defer)
- [ ] 1.2 Record owner, rationale, and definition of done vs #173

## 2. If A, B, or C

- [ ] 2.1 Compatibility matrix: base URL, auth, streaming, paths (min `POST /v1/responses` + Agents SDK smoke paths)
- [ ] 2.2 Golden-path Agents SDK test recipe (pinned SDK version)
- [ ] 2.3 Negative test for explicitly unsupported paths
- [ ] 2.4 Regression guard for chat completions
- [ ] 2.5 If **C**: policy for method allowlist, size caps, denylist
- [ ] 2.6 Draft `openspec/specs/openai-responses-compat/spec.md`

## 3. Documentation

- [ ] 3.1 “OpenAI compatibility” section with supported slice and validation pins (non-D)
- [ ] 3.2 If **D**: update README/docs to remove false implications

## 4. Close

- [ ] 4.1 Archive decision; open implementation change if A/B/C chosen
