#!/usr/bin/env bash

set -euo pipefail

repo_root="$(
  cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P
)"

DODECA_VERSION="${DODECA_VERSION:-v0.14.2}"
DODECA_INSTALL_DIR="${repo_root}/.ci-cache/dodeca-install/bin"

cd "${repo_root}"

export DODECA_CELL_PATH="${DODECA_INSTALL_DIR}"
export PATH="${repo_root}/docs/node_modules/.bin:${DODECA_INSTALL_DIR}:${PATH}"

install_dodeca_release() {
  local installer_path

  rm -rf "${DODECA_INSTALL_DIR}"
  mkdir -p "${DODECA_INSTALL_DIR}" "${repo_root}/.ci-cache"
  installer_path="${repo_root}/.ci-cache/dodeca-installer.sh"

  curl --proto '=https' --tlsv1.2 -LsSf \
    "https://github.com/bearcove/dodeca/releases/download/${DODECA_VERSION}/dodeca-installer.sh" \
    -o "${installer_path}"

  DODECA_VERSION="${DODECA_VERSION}" \
    DODECA_INSTALL_DIR="${DODECA_INSTALL_DIR}" \
    sh "${installer_path}"
}

npm install --prefix docs --no-audit --no-fund

if ! command -v helm >/dev/null 2>&1; then
  mise install helm@latest
  helm_bin="$(mise exec helm@latest -- sh -lc 'command -v helm')"
  ln -sf "${helm_bin}" /usr/local/bin/helm
fi

install_dodeca_release

command -v helm
command -v ddc
command -v tailwindcss
