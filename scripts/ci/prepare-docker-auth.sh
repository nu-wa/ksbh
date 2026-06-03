#!/usr/bin/env bash

set -euo pipefail

source_docker_config="${DOCKER_CONFIG:-/root/.docker}"
writable_docker_config="${KSBH_WRITABLE_DOCKER_CONFIG:-/tmp/ksbh-docker-config}"
writable_buildx_config="${KSBH_WRITABLE_BUILDX_CONFIG:-/tmp/ksbh-docker-buildx}"

mkdir -p "${writable_docker_config}" "${writable_buildx_config}"

if [ -f "${source_docker_config}/config.json" ]; then
  cp "${source_docker_config}/config.json" "${writable_docker_config}/config.json"
fi

export DOCKER_CONFIG="${writable_docker_config}"
export BUILDX_CONFIG="${writable_buildx_config}"

if [ -n "${HARBOR_HOST:-}" ] && [ -n "${HARBOR_USERNAME:-}" ] && [ -n "${HARBOR_PASSWORD:-}" ]; then
  echo "${HARBOR_PASSWORD}" | docker login "${HARBOR_HOST}" --username "${HARBOR_USERNAME}" --password-stdin
  echo "Prepared Docker auth for ${HARBOR_HOST}"
fi

persist_env() {
  local env_file="$1"

  if [ -n "${env_file}" ] && [ -w "${env_file}" ]; then
    {
      printf 'DOCKER_CONFIG=%s\n' "${DOCKER_CONFIG}"
      printf 'BUILDX_CONFIG=%s\n' "${BUILDX_CONFIG}"
    } >> "${env_file}"
  fi
}

persist_env "${GITHUB_ENV:-}"
persist_env "${FORGEJO_ENV:-}"

if [ -z "${GITHUB_ENV:-}" ] && [ -z "${FORGEJO_ENV:-}" ]; then
  {
    printf 'DOCKER_CONFIG=%s\n' "${DOCKER_CONFIG}"
    printf 'BUILDX_CONFIG=%s\n' "${BUILDX_CONFIG}"
  } > "${writable_docker_config}/env"
fi
