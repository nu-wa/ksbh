#
# syntax=docker/dockerfile:1.7
#
FROM rust:1.92.0-slim-bookworm AS builder

WORKDIR /build

RUN apt-get update -y && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    build-essential \
    cmake \
  && rm -rf /var/lib/apt/lists/*

# Install sccache as a separate layer so subsequent source changes don't bust
# the install layer cache.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo install sccache --locked

COPY ./crates /build

# `host-registry` build context is provided by build-release-image and points
# at the host's cargo registry, so a cache-miss release build skips
# re-downloading every crate. When the build context is missing, BuildKit
# fails — the build script always provides it (falling back to an empty tmp
# dir when the host has no cargo registry yet).
#
# `sccache_env` secret is only set when the caller (build-rust) has the
# relevant env vars; required=false so a developer build without sccache
# creds still works.
RUN --mount=type=bind,from=host-registry,source=/,target=/usr/local/cargo/registry,rw=true \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=secret,id=sccache_env,required=false \
    <<'EOF'
set -eu
if [ -f /run/secrets/sccache_env ]; then
  set -a
  . /run/secrets/sccache_env
  set +a
  export RUSTC_WRAPPER="$(command -v sccache)"
fi
cargo build --release \
  -p ksbh \
  -p http_to_https \
  -p proof-of-work \
  -p rate-limit \
  -p robots-txt \
  -p oidc
EOF

FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update -y && apt-get install -y --no-install-recommends ca-certificates libssl3 openssl && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /app/modules /usr/lib/ksbh/modules /app/config

COPY --from=builder /build/target/release/ksbh /app/ksbh
COPY --from=builder /build/target/release/*.so /usr/lib/ksbh/modules/
COPY ./docker/build/release-entrypoint.sh /app/release-entrypoint.sh

RUN chmod +x /app/ksbh /app/release-entrypoint.sh

ENTRYPOINT ["/app/release-entrypoint.sh"]
