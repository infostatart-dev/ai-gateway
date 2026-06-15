## Contributing

Thanks for your interest in contributing to **AI Gateway** (Infostart Lab fork).

Please note that this project is released with a [Contributor Code of Conduct](CODE_OF_CONDUCT.md). By participating in this project you agree to abide by its terms.

## Issues and PRs

If you have suggestions for how this project could be improved, or want to report a bug, open an issue. Pull requests are welcome. For large changes, open an issue first to discuss the approach.

## Submitting a pull request

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) 1.91+
- [Docker](https://docs.docker.com/get-docker/) and [Docker Compose](https://docs.docker.com/compose/install/) (optional)

### Setup

1. [Fork](https://github.com/infostatart-dev/ai-gateway/fork) and clone the repository.
2. Copy environment template:

   ```bash
   cp .env.template .env
   ```

   Set `AI_GATEWAY_CREDENTIAL_*` variables for providers you plan to test.

3. Start docker compose (optional):

   ```bash
   cd infrastructure && docker compose up -d && cd ..
   ```

4. Start the gateway from the repository root:

   ```bash
   cargo run

   # Or with a dev config file:
   cargo rl
   ```

5. Run tests:

   ```bash
   cargo run -p test
   cargo test --tests --all-features
   ```

6. Build:

   ```bash
   cargo build
   cargo build --release
   ```

7. Create a branch: `git checkout -b my-branch-name`.
8. Make your change, add tests, and ensure tests pass.
9. Commit using [conventional commit format](https://www.conventionalcommits.org/en/v1.0.0/).
10. Push to your fork and [submit a pull request](https://github.com/infostatart-dev/ai-gateway/compare).

Tips for accepted PRs:

- Write and update tests.
- Keep changes focused; split unrelated work into separate PRs.
- Write clear commit messages: why the change is needed, what changed, relevant links.

Work-in-progress pull requests are welcome for early feedback.
