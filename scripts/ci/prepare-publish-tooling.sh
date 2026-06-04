#!/usr/bin/env bash

set -euo pipefail

repo_root="$(
  cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P
)"

DODECA_VERSION="${DODECA_VERSION:-v0.14.2}"
DODECA_SHA="${DODECA_SHA:-4c25f36b64b9ec5aab5612a924b1264defa141d6}"

cd "${repo_root}"

export PATH="${repo_root}/docs/node_modules/.bin:${repo_root}/.ci-cache/dodeca-install/bin:${PATH}"

npm install --prefix docs --no-audit --no-fund

if ! command -v helm >/dev/null 2>&1; then
  mise install helm@latest
  helm_bin="$(mise exec helm@latest -- sh -lc 'command -v helm')"
  ln -sf "${helm_bin}" /usr/local/bin/helm
fi

if ! command -v ddc >/dev/null 2>&1; then
  rm -rf .ci-cache/dodeca-src
  mkdir -p .ci-cache/dodeca-install .ci-cache/dodeca-target
  git clone --depth 50 --branch "${DODECA_VERSION}" https://github.com/bearcove/dodeca .ci-cache/dodeca-src
  (
    cd .ci-cache/dodeca-src
    git checkout "${DODECA_SHA}"
    export CARGO_TARGET_DIR="${repo_root}/.ci-cache/dodeca-target"
    cd crates/dodeca-devtools
    wasm-pack build --target web
    cd ../dodeca-search-wasm
    wasm-pack build --target web
    cargo install --path ../dodeca --locked --root "${repo_root}/.ci-cache/dodeca-install"
  )
fi

command -v helm
command -v ddc
command -v tailwindcss
