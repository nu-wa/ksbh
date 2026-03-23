FROM rust:bookworm

WORKDIR /build

COPY ./crates /build

RUN apt-get update -y && apt-get install -y pkg-config libssl-dev build-essential cmake

RUN rustup toolchain install nightly --profile minimal
RUN rustup component add miri rust-src --toolchain nightly
RUN cargo +nightly miri setup
RUN cargo +nightly miri setup --print-sysroot > /miri-sysroot

CMD ["bash", "-c", "export MIRI_SYSROOT=\"$(cat /miri-sysroot)\"; /usr/local/cargo/bin/cargo +nightly miri test --manifest-path /build/Cargo.toml -p ksbh-modules-sdk --test ffi_miri -- --nocapture --test-threads=1"]
