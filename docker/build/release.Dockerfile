#
# syntax=docker/dockerfile:1.7
#
FROM rust:1.92.0-slim-bookworm AS builder

WORKDIR /build

COPY ./crates /build

RUN apt-get update -y && apt-get install -y pkg-config libssl-dev build-essential cmake

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    <<EOF
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

RUN apt-get update -y && apt-get install -y ca-certificates libssl3 openssl && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /app/modules /usr/lib/ksbh/modules /app/config

COPY --from=builder /build/target/release/ksbh /app/ksbh
COPY --from=builder /build/target/release/*.so /usr/lib/ksbh/modules/
COPY ./docker/build/release-entrypoint.sh /app/release-entrypoint.sh

RUN chmod +x /app/ksbh /app/release-entrypoint.sh

ENTRYPOINT ["/app/release-entrypoint.sh"]
