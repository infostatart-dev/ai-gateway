# DeepSeek Web provider

The `deepseek-web` provider routes chat completions through a **browser session**
(`localStorage.userToken` on chat.deepseek.com), not the DeepSeek API key.

## Enable

1. Add session path to `dev/secrets.local.yaml` (see
   [`dev/secrets.local.example.yaml`](../dev/secrets.local.example.yaml)):

   ```yaml
   credentials:
     deepseek-web-default:
       session-file: dev/deepseek-session.json
   ```

2. Create the session (requires `deepseek-login` feature):

   **Interactive browser login:**

   ```bash
   cargo run --features deepseek-login -- deepseek login
   ```

   **Import token from DevTools:**

   ```bash
   cargo run --features deepseek-login -- deepseek import \
     --token 'your-userToken-value'
   ```

   Token location: DevTools → Application → Local Storage →
   `chat.deepseek.com` → `userToken` (plain string or JSON `{"value":"..."}`).

3. Restart the gateway. When the session file exists, `deepseek-web` joins
   **autodefault** (cost-class routing — see [routing.md](routing.md)).

## Smoke tests

Verify token exchange:

```bash
cargo run --features deepseek-login -- deepseek probe
```

Optional completion:

```bash
cargo run --features deepseek-login -- deepseek probe \
  --query 'Reply with exactly one word: OK'
```

Structured output smoke (`json_schema`):

```bash
cargo run --features deepseek-login -- deepseek probe --structured-output
```

Context limit calibration (binary search single-prompt size):

```bash
cargo run --features deepseek-login -- deepseek probe --context-limit
```

## Models

| Model | JSON schema | Context (catalog) | Notes |
|-------|-------------|-------------------|-------|
| `deepseek-web/deepseek-chat` | yes | 128000 | Standard chat |
| `deepseek-web/deepseek-reasoner` | yes | 128000 | `reasoning_content` + JSON `content` |

Tools are **not** supported (`supports-tools: false`).

## Structured output

Both models accept OpenAI `response_format`:

- `json_schema` (including `strict: true`) — schema injected into the prompt;
  assistant **`content`** validated (not `reasoning_content` on reasoner)
- `json_object` — JSON-only instruction; parse validation only

Non-streaming requests retry the **final turn** up to two times on invalid JSON
or schema mismatch. Upload/context turns are not schema-validated.

Example:

```bash
curl -sS http://127.0.0.1:8080/v1/chat/completions \
  -H "Authorization: Bearer $AI_GATEWAY_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "deepseek-web/deepseek-chat",
    "response_format": {
      "type": "json_schema",
      "json_schema": {
        "name": "status",
        "strict": true,
        "schema": {
          "type": "object",
          "properties": { "status": { "type": "string" } },
          "required": ["status"],
          "additionalProperties": false
        }
      }
    },
    "messages": [{ "role": "user", "content": "Say ok." }]
  }'
```

## Long context and chunk upload

Oversized payloads use the shared `web-message-budget` planner:

- **128000** token input budget (default until `--context-limit` probe says otherwise)
- **45000** tokens per upload part (conservative vs ChatGPT Web 90k)
- One `chat_session_id` per gateway request; schema only on the final turn
- **PoW cache** (45s TTL) reuses proof-of-work answers across upload turns

Each upload/final turn consumes one **pacing** slot (`deepseek-web` RPM limits).
Route trace logs `deepseek_web_turns`, `deepseek_web_upload_parts`, and
`deepseek_web_pow_cache_hits`.

## Session maintenance

Sessions expire when DeepSeek invalidates `userToken`. Symptoms: HTTP 401,
`invalid_session` in error body. Re-run `deepseek login` or `deepseek import`.

See also: [credentials.md](credentials.md), [routing.md](routing.md).
