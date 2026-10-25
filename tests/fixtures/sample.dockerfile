# Stage 1: Build
FROM rust:1.78-slim AS builder

ARG APP_VERSION=1.0.0
ENV CARGO_HOME=/usr/local/cargo

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/

RUN apt-get update && apt-get install -y pkg-config libssl-dev \
    && cargo build --release

EXPOSE 8080

# Stage 2: Runtime
FROM debian:bookworm-slim AS runtime

LABEL maintainer="dev@example.com"
LABEL version="${APP_VERSION}"

ENV APP_PORT=8080
ENV LOG_LEVEL=info

RUN apt-get update && apt-get install -y ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/myapp /usr/local/bin/myapp

HEALTHCHECK --interval=30s --timeout=3s \
    CMD curl -f http://localhost:8080/health || exit 1

USER nobody

ENTRYPOINT ["myapp"]
CMD ["--port", "8080"]
