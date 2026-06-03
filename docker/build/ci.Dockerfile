#
# syntax=docker/dockerfile:1.7
#
FROM ghcr.io/catthehacker/ubuntu:act-latest

ARG TARGETARCH
ARG DODECA_VERSION=v0.14.2

ENV DEBIAN_FRONTEND=noninteractive
ENV PATH="/opt/ksbh-docs-tools/node_modules/.bin:/opt/ksbh-playwright/node_modules/.bin:/root/.deno/bin:/root/.local/bin:/root/.cargo/bin:${PATH}"
ENV PLAYWRIGHT_BROWSERS_PATH=/ms-playwright
ENV RUSTUP_HOME=/root/.rustup
ENV CARGO_HOME=/root/.cargo

RUN apt-get update -y \
  && asound_pkg="libasound2" \
  && if apt-cache show libasound2t64 >/dev/null 2>&1; then asound_pkg="libasound2t64"; fi \
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
    python3 \
    nodejs \
    npm \
    libnss3 \
    libnspr4 \
    libatk1.0-0 \
    libatk-bridge2.0-0 \
    libcups2 \
    libdrm2 \
    libdbus-1-3 \
    libxkbcommon0 \
    libxcomposite1 \
    libxdamage1 \
    libxfixes3 \
    libxrandr2 \
    libgbm1 \
    "${asound_pkg}" \
    libatspi2.0-0 \
    libxshmfence1 \
    fonts-liberation \
  && rm -rf /var/lib/apt/lists/*

RUN echo "y" | sh -c "$(curl -fsSL https://mise.run)" \
  && ln -sf /root/.local/bin/mise /usr/local/bin/mise

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | bash -s -- -y

RUN rustup toolchain install 1.92.0 \
  && rustup default 1.92.0 \
  && rustup target add wasm32-unknown-unknown

RUN cargo install sccache --locked
RUN cargo install wasm-pack --locked

RUN curl -fsSL "https://kind.sigs.k8s.io/dl/v0.31.0/kind-linux-${TARGETARCH}" -o /usr/local/bin/kind \
  && chmod +x /usr/local/bin/kind

RUN curl -fsSL "https://get.helm.sh/helm-v3.18.4-linux-${TARGETARCH}.tar.gz" -o /tmp/helm.tgz \
  && tar -C /tmp -xzf /tmp/helm.tgz \
  && mv "/tmp/linux-${TARGETARCH}/helm" /usr/local/bin/helm \
  && rm -rf /tmp/helm.tgz "/tmp/linux-${TARGETARCH}"

RUN curl -fsSL "https://dl.k8s.io/release/v1.33.1/bin/linux/${TARGETARCH}/kubectl" -o /usr/local/bin/kubectl \
  && chmod +x /usr/local/bin/kubectl

RUN curl -fsSL https://deno.land/install.sh | sh -s -- v2.3.7 \
  && ln -sf /root/.deno/bin/deno /usr/local/bin/deno

RUN git clone --depth 1 --branch "${DODECA_VERSION}" https://github.com/bearcove/dodeca /tmp/dodeca \
  && cd /tmp/dodeca/crates/dodeca-devtools \
  && wasm-pack build --target web \
  && cd /tmp/dodeca/crates/dodeca-search-wasm \
  && wasm-pack build --target web \
  && cargo install --path /tmp/dodeca/crates/dodeca --locked \
  && rm -rf /tmp/dodeca

WORKDIR /opt/ksbh-docs-tools
COPY docs/package.json /opt/ksbh-docs-tools/package.json
RUN npm install --no-audit --no-fund

WORKDIR /opt/ksbh-playwright
COPY tests/playwright/package.json tests/playwright/package-lock.json /opt/ksbh-playwright/
RUN npm ci --no-audit --no-fund \
  && npx playwright install chromium
