#
# syntax=docker/dockerfile:1.7
#
FROM ghcr.io/catthehacker/ubuntu:act-latest

ARG TARGETARCH
ARG DODECA_VERSION=v0.14.2

ENV DEBIAN_FRONTEND=noninteractive
ENV PATH="/opt/ksbh-docs-tools/node_modules/.bin:/opt/ksbh-playwright/node_modules/.bin:/root/.deno/bin:/root/.local/bin:/root/.cargo/bin:${PATH}"
ENV RUSTUP_HOME=/root/.rustup
ENV CARGO_HOME=/root/.cargo

RUN --mount=type=cache,target=/var/cache/apt/archives,sharing=locked \
  --mount=type=cache,target=/var/lib/apt,sharing=locked \
  apt-get update -y \
  && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    zstd \
    iproute2 \
    git \
    docker.io \
    build-essential \
    cmake \
    pkg-config \
    libssl-dev \
    mold \
    python3 \
    nodejs \
    npm \
    jq \
    unzip \
    libnss3 \
    libnspr4 \
    libatk1.0-0t64 \
    libatk-bridge2.0-0t64 \
    libcups2t64 \
    libdrm2 \
    libxkbcommon0 \
    libxcomposite1 \
    libxdamage1 \
    libxfixes3 \
    libxrandr2 \
    libgbm1 \
    libasound2t64 \
    xvfb \
  && rm -rf /var/lib/apt/lists/*

RUN echo "y" | MISE_VERSION=v2026.5.15 sh -c "$(curl -fsSL https://mise.run)" \
  && ln -sf /root/.local/bin/mise /usr/local/bin/mise

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | bash -s -- -y

RUN --mount=type=cache,target=/root/.rustup/downloads,sharing=locked \
  rustup toolchain install 1.92.0 \
  && rustup default 1.92.0 \
  && rustup target add wasm32-unknown-unknown \
  && rustup toolchain install nightly --component miri --component rust-src --profile minimal

RUN --mount=type=cache,target=/root/.cargo/registry,sharing=locked \
  --mount=type=cache,target=/root/.cargo/git,sharing=locked \
  cargo install sccache --locked

RUN --mount=type=cache,target=/root/.cargo/registry,sharing=locked \
  --mount=type=cache,target=/root/.cargo/git,sharing=locked \
  cargo install wasm-pack --locked

RUN targetarch="${TARGETARCH:-$(dpkg --print-architecture)}" \
  && curl -fsSL "https://kind.sigs.k8s.io/dl/v0.31.0/kind-linux-${targetarch}" -o /usr/local/bin/kind \
  && chmod +x /usr/local/bin/kind

RUN targetarch="${TARGETARCH:-$(dpkg --print-architecture)}" \
  && curl -fsSL "https://get.helm.sh/helm-v3.18.4-linux-${targetarch}.tar.gz" -o /tmp/helm.tgz \
  && tar -C /tmp -xzf /tmp/helm.tgz \
  && mv "/tmp/linux-${targetarch}/helm" /usr/local/bin/helm \
  && rm -rf /tmp/helm.tgz "/tmp/linux-${targetarch}"

RUN targetarch="${TARGETARCH:-$(dpkg --print-architecture)}" \
  && curl -fsSL "https://dl.k8s.io/release/v1.33.1/bin/linux/${targetarch}/kubectl" -o /usr/local/bin/kubectl \
  && chmod +x /usr/local/bin/kubectl

RUN curl -fsSL https://deno.land/install.sh | sh -s -- v2.3.7 \
  && ln -sf /root/.deno/bin/deno /usr/local/bin/deno

WORKDIR /opt/ksbh-mise
COPY mise.toml /opt/ksbh-mise/mise.toml
RUN --mount=type=cache,target=/root/.cache/mise,sharing=locked \
  mise trust /opt/ksbh-mise/mise.toml \
  && mise install

RUN bash -euo pipefail -c ' \
    curl --proto "=https" --tlsv1.2 -LsSf \
      "https://github.com/bearcove/dodeca/releases/download/${DODECA_VERSION}/dodeca-installer.sh" \
      -o /tmp/dodeca-installer.sh; \
    DODECA_VERSION="${DODECA_VERSION}" DODECA_INSTALL_DIR=/root/.cargo/bin sh /tmp/dodeca-installer.sh; \
    test -x /root/.cargo/bin/ddc; \
    rm -f /tmp/dodeca-installer.sh \
  '

WORKDIR /opt/ksbh-docs-tools
COPY docs/package.json /opt/ksbh-docs-tools/package.json
RUN --mount=type=cache,target=/root/.npm,sharing=locked \
  npm install --no-audit --no-fund

WORKDIR /opt/ksbh-playwright
COPY tests/playwright/package.json tests/playwright/package-lock.json /opt/ksbh-playwright/
RUN --mount=type=cache,target=/root/.npm,sharing=locked \
  npm ci --no-audit --no-fund

RUN --mount=type=cache,target=/root/.cache/ms-playwright,sharing=locked \
  npx playwright install --with-deps

RUN command -v mise \
  && command -v rustc \
  && command -v cargo \
  && command -v sccache \
  && command -v wasm-pack \
  && command -v ddc \
  && command -v kind \
  && command -v helm \
  && command -v kubectl \
  && command -v docker \
  && command -v node \
  && command -v npm \
  && command -v deno \
  && command -v git \
  && command -v python3 \
  && command -v cmake \
  && command -v pkg-config \
  && command -v mold \
  && command -v playwright \
  && cd /opt/ksbh-mise \
  && mise where rust \
  && mise exec -- rustc --version \
  && sccache --version
