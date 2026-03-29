#
# syntax=docker/dockerfile:1.7
#
FROM ghcr.io/catthehacker/ubuntu:act-latest

ENV DEBIAN_FRONTEND=noninteractive
ENV PLAYWRIGHT_VERSION=1.54.2

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

RUN if ! command -v rustup >/dev/null 2>&1; then \
      curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal; \
    fi \
  && /root/.cargo/bin/rustup toolchain install nightly --profile minimal \
  && /root/.cargo/bin/rustup component add miri rust-src --toolchain nightly \
  && /root/.cargo/bin/cargo +nightly miri setup

RUN npm install --global "@playwright/test@${PLAYWRIGHT_VERSION}" \
  && playwright install chromium

ENV PATH="/root/.local/bin:/root/.cargo/bin:${PATH}"
