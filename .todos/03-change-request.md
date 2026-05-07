# Change Request: Documentation link hygiene and README accuracy

## Context

- Upstream reports **broken Quick Start and Introduction links** in `README.md` ([Helicone/ai-gateway#297](https://github.com/Helicone/ai-gateway/issues/297)); users expect those entry points to resolve to live Helicone AI Gateway documentation.
- This fork’s tree still contains **additional documentation drift** discovered in review: **stale repository names** (`helicone-router` clone/fork/PR instructions vs `ai-gateway`), a **likely typo** in a GitHub releases badge URL (`aia-gateway`), **sentence-trailing punctuation** absorbed into link targets (e.g. `typos.`), **Markdown-truncated** badge URLs when naïvely extracting `https://…`, and **template fragments** in prose that are not real URLs (`https://<bucket`, `https://s3.<region`). Scope is **operator- and contributor-facing** docs and badges, not runtime gateway behavior.

## What must be done

1. **Fix README “front door” links** called out in upstream: **Quickstart** and **Introduction** (and any sibling nav targets in the same README block) must **HTTP 200** to the intended `docs.helicone.ai` pages or be updated to the **current canonical paths** Helicone publishes; track resolution against issue #297.
2. **Normalize repository identity** in English contributor docs (`CONTRIBUTING.md`, `DEVELOPMENT.md`, and any other `git clone` / “open a PR” instructions): replace **`Helicone/helicone-router`** (and related compare/fork URLs) with **`Helicone/ai-gateway`** (or this fork’s canonical Git remote if product requires a fork-first story)—**one** consistent story per file.
3. **Audit and repair high-signal external links** in root-level Markdown and badges: correct **obvious typos** (e.g. releases URL), strip **trailing punctuation** from link targets in prose, ensure **shields.io** and similar badge `href`s are **complete** (valid in rendered README), and either **complete or remove** non-URL **placeholders** in user-visible doc (S3/bucket examples should be clearly marked as placeholders or given a full example URL policy).
4. **Optional but recommended**: maintain a **small recurring check** (manual checklist or CI link checker scoped to `*.md` and excluding lockfile noise) so new docs do not reintroduce the same failure class—**policy and owner** must be named if adopted.

## Expected end state

- **README**: Quickstart + Introduction (and linked “first hour” doc targets from the same header/footer blocks) work for a **cold reader** without 404s; issue #297 can be **closed on upstream** if this fork contributes back, or **mirrored closed** with a note if only fork is updated.
- **CONTRIBUTING / DEVELOPMENT**: clone and PR instructions reference the **correct repository**; no `helicone-router` leftovers unless explicitly documented as historical redirect.
- **Link inventory**: either (a) no known **broken or misleading** URLs in the agreed file set, or (b) a short **EXCEPTIONS** list with rationale (e.g. intentionally pointing to archived resources)—no silent “looks like URL, is garbage” fragments in user-facing pages.

## Notes

- **Direction** matches upstream intent in #297 (trustworthy onboarding); **correctness** is mostly **URL and repo identity hygiene**, not product feature work.
- **Lockfiles** (`uv.lock`, `yarn.lock`, `Cargo.lock`, etc.) contain hundreds of third-party URLs; **out of scope** for human “README fix” unless a separate CR adopts automated policy for those artifacts.
- **Implementation** (exact edits per line, CI wiring) is **out of scope** for this CR; this file defines **what “done” means** for documentation and links.
