#!/usr/bin/env bash

set -euo pipefail

repo_root="$(
  cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P
)"

local_env_file="${KSBH_CI_LOCAL_ENV_FILE:-${repo_root}/.env.ci.local}"
if [ -f "${local_env_file}" ]; then
  set -a
  # shellcheck disable=SC1090
  . "${local_env_file}"
  set +a
fi

ci_image="${CI_BASE_IMAGE:-ksbh-ci:local}"
docker_auth_path="${CI_LOCAL_DOCKER_AUTH_PATH:-${DOCKER_CONFIG:-${HOME}/.docker}}"
ci_platform="${CI_LOCAL_PLATFORM:-}"
cargo_registry_cache="${KSBH_LOCAL_CARGO_REGISTRY:-}"
cargo_git_cache="${KSBH_LOCAL_CARGO_GIT:-}"
sccache_dir="${KSBH_LOCAL_SCCACHE_DIR:-}"
miri_rustup_home="${KSBH_MIRI_RUSTUP_HOME:-}"
miri_cargo_home="${KSBH_MIRI_CARGO_HOME:-}"
miri_target_dir="${KSBH_MIRI_TARGET_DIR:-}"

docker_args=(
  run
  --rm
  -v "${repo_root}:/workspace"
  -w /workspace
  -v /var/run/docker.sock:/var/run/docker.sock
  -e CI=1
  -e LOCAL_PUBLISH="${LOCAL_PUBLISH:-false}"
  -e CI_BASE_IMAGE="${ci_image}"
)

if [ -n "${ci_platform}" ]; then
  docker_args+=(--platform "${ci_platform}")
fi

if [ -d "${docker_auth_path}" ]; then
  docker_args+=(-v "${docker_auth_path}:/root/.docker:ro")
fi

if [ -n "${cargo_registry_cache}" ]; then
  mkdir -p "${cargo_registry_cache}"
  docker_args+=(-v "${cargo_registry_cache}:/root/.cargo/registry")
fi

if [ -n "${cargo_git_cache}" ]; then
  mkdir -p "${cargo_git_cache}"
  docker_args+=(-v "${cargo_git_cache}:/root/.cargo/git")
fi

if [ -n "${sccache_dir}" ]; then
  mkdir -p "${sccache_dir}"
  docker_args+=(-v "${sccache_dir}:/root/.cache/sccache")
fi

if [ -n "${miri_rustup_home}" ]; then
  mkdir -p "${miri_rustup_home}"
  docker_args+=(-v "${miri_rustup_home}:${miri_rustup_home}")
fi

if [ -n "${miri_cargo_home}" ]; then
  mkdir -p "${miri_cargo_home}"
  docker_args+=(-v "${miri_cargo_home}:${miri_cargo_home}")
fi

if [ -n "${miri_target_dir}" ]; then
  mkdir -p "${miri_target_dir}"
  docker_args+=(-v "${miri_target_dir}:${miri_target_dir}")
fi

for passthrough_var in \
  HARBOR_HOST \
  DOCKERHUB_REPOSITORY \
  DOCKERHUB_USERNAME \
  DOCKERHUB_TOKEN \
  KSBH_RELEASE_IMAGE_REPOSITORY \
  KSBH_RELEASE_IMAGE_TAG \
  KSBH_RELEASE_IMAGE_CACHE_REPO \
  KSBH_DOCS_SITE_IMAGE \
  KSBH_DOCS_SITE_CACHE_REPO \
  KSBH_CHARTS_SITE_IMAGE \
  KSBH_CHARTS_SITE_CACHE_REPO \
  KSBH_E2E_WEBSOCKET_IMAGE \
  KSBH_E2E_WEBSOCKET_CACHE_REPO \
  KIND_CLUSTER_CONFIG \
  KIND_CLUSTER_NAME \
  KSBH_KIND_API_SERVER_HOST \
  KSBH_KIND_API_SERVER_PORT \
  KSBH_CI_HOST \
  SCCACHE_BUCKET \
  SCCACHE_ENDPOINT \
  SCCACHE_REGION \
  SCCACHE_S3_USE_SSL \
  SCCACHE_S3_ENABLE_VIRTUAL_HOST_STYLE \
  AWS_ACCESS_KEY_ID \
  AWS_SECRET_ACCESS_KEY \
  RUSTC_WRAPPER \
  KSBH_MIRI_RUSTUP_HOME \
  KSBH_MIRI_CARGO_HOME \
  KSBH_MIRI_TARGET_DIR
do
  if [ -n "${!passthrough_var:-}" ]; then
    docker_args+=(-e "${passthrough_var}=${!passthrough_var}")
  fi
done

exec docker "${docker_args[@]}" "${ci_image}" "$@"
