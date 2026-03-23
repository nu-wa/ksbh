#
# syntax=docker/dockerfile:1.7
#
FROM ghcr.io/catthehacker/ubuntu:act-latest

ENV DEBIAN_FRONTEND=noninteractive

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

ENV PATH="/root/.local/bin:${PATH}"
