# syntax=docker/dockerfile:1
FROM lukemathwalker/cargo-chef:latest-rust-1.91.1-bookworm AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --bin ai-gateway --recipe-path recipe.json

FROM chef AS builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev cmake clang libclang-dev build-essential \
    && rm -rf /var/lib/apt/lists/*
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release -p ai-gateway \
    && cp /app/target/release/ai-gateway /tmp/ai-gateway

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl openssl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /tmp/ai-gateway /usr/local/bin/ai-gateway
COPY ai-gateway/config/helicone-cloud.yaml /etc/ai-gateway/helicone-cloud.yaml
EXPOSE 8080
CMD ["/usr/local/bin/ai-gateway"]
