# AI Gateway local demo

Instructions for running a quick local demo.

## Steps

1. Set up your environment as described in [DEVELOPMENT.md](DEVELOPMENT.md).
   Configure at least one `AI_GATEWAY_CREDENTIAL_*` variable in `.env`.

2. Run the gateway:

   ```bash
   cargo run
   ```

3. Send a test request:

   ```bash
   cargo run -p test
   ```

4. Verify the response in your terminal (HTTP 200 and model output).

For more examples, see [examples/python/README.md](examples/python/README.md) and
[examples/typescript/README.md](examples/typescript/README.md).
