# syntax=docker/dockerfile:1.7
#
# Multi-stage build: cargo-chef caches dep compilation, bun builds the SPA via build.rs,
# distroless cc runs the stripped binary as nonroot. All base images are upstream-official:
# `rust` (Docker Official), `gcr.io/distroless/cc-debian12` (Google).

# Chef base: official rust image + cargo-chef + bun. cargo-chef gives us dep-graph caching;
# bun is installed from the upstream install script (the bun project does not publish a Docker
# Official Image, and we want only verified base images here).
FROM rust:bookworm AS chef
WORKDIR /app
RUN apt-get update \
 && apt-get install -y --no-install-recommends curl ca-certificates unzip \
 && rm -rf /var/lib/apt/lists/* \
 && curl -fsSL https://bun.sh/install | bash \
 && ln -s /root/.bun/bin/bun /usr/local/bin/bun \
 && cargo install cargo-chef --locked --version ^0.1

# Plan: emit a recipe.json describing the dep graph. Cached as long as Cargo.{toml,lock} are stable.
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Build: cook deps from recipe (cached layer), then compile the workspace + SPA.
# Cache mounts (per-arch, so amd64 and arm64 don't collide):
#   - cargo registry/git: dep source cache (shared across all rust builds)
#   - target/: compiled artifacts; chef-cook + workspace build write here, persisted across builds
#   - bun install cache + node_modules: speed up the build.rs SPA pipeline
FROM chef AS builder
ARG TARGETARCH
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=cargo-registry-${TARGETARCH} \
    --mount=type=cache,target=/usr/local/cargo/git,id=cargo-git-${TARGETARCH} \
    --mount=type=cache,target=/app/target,id=cargo-target-${TARGETARCH},sharing=locked \
    cargo chef cook --release --recipe-path recipe.json
COPY . .
# build.rs only runs the bun pipeline in release mode or when this is set; explicit is safer.
ENV SUDORATIO_BUILD_WEB=1
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=cargo-registry-${TARGETARCH} \
    --mount=type=cache,target=/usr/local/cargo/git,id=cargo-git-${TARGETARCH} \
    --mount=type=cache,target=/app/target,id=cargo-target-${TARGETARCH},sharing=locked \
    --mount=type=cache,target=/root/.bun/install/cache,id=bun-cache-${TARGETARCH} \
    --mount=type=cache,target=/app/web/node_modules,id=node-modules-${TARGETARCH},sharing=locked \
    cargo build --release --bin sudoratio-server \
    && strip target/release/sudoratio-server \
    && cp target/release/sudoratio-server /sudoratio-server \
    && mkdir -p /data-template

# Distroless cc keeps libc/libgcc/libssl for reqwest's native-tls and nothing else.
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime
LABEL org.opencontainers.image.title="sudoratio" \
      org.opencontainers.image.description="Self-hosted BitTorrent tracker-protocol simulator and research toolkit with a modern web UI." \
      org.opencontainers.image.authors="Adnan Ahmad <viperadnan@gmail.com>" \
      org.opencontainers.image.url="https://github.com/viperadnan-git/sudoratio" \
      org.opencontainers.image.source="https://github.com/viperadnan-git/sudoratio" \
      org.opencontainers.image.documentation="https://github.com/viperadnan-git/sudoratio#readme" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.vendor="viperadnan"
COPY --from=builder /sudoratio-server /usr/local/bin/sudoratio-server
# Pre-create /data owned by `nonroot` (distroless ships no `mkdir`/`chown`); a fresh anonymous
# or named volume mounted here inherits this ownership on first run.
COPY --from=builder --chown=nonroot:nonroot /data-template /data
VOLUME ["/data"]
EXPOSE 8787
USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/sudoratio-server"]
CMD ["--config-dir", "/data", "--listen", "0.0.0.0:8787"]
