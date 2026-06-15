## Decision status

**Status:** pending  
**Owner:** (assign)

## Options

| Option | Summary |
|--------|---------|
| **Adopt** | Port/re-implement following fork provider invariants |
| **Defer** | Document trigger and revisit cadence |
| **Partial** | Reduced contract (e.g. config-only) with explicit product agreement |

## Expected end state

- Written decision with definition of done for chosen tier
- If **adopt**: operators enable Nebius via standard config/env; CI green for new tests; no undocumented auth/base-URL behavior
- If **defer/partial**: single place states what users must not expect

## Notes

- Re-validate “no breaking changes” claim from #299 against **this fork’s** diff from upstream.
- Implementation order inside adopt is a separate change; this change fixes **whether** and **what bar**.
