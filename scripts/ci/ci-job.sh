#!/usr/bin/env bash

set -euo pipefail

job_name="${1:-}"
if [ -z "${job_name}" ]; then
  echo "usage: $0 <build-rust|test-binary|test-modules|test-k8s|test-miri|build-docs|helm-artifacts>" >&2
  exit 1
fi
shift

case "${job_name}" in
  build-rust)
    command_string='mise run compile-test-binaries'
    ;;
  test-binary)
    command_string='export KSBH_CI_USE_PREBUILT=false; mise run test-binary'
    ;;
  test-modules)
    command_string='export KSBH_CI_USE_PREBUILT=false; mise run test-modules-smoke && mise run test-unhappy'
    ;;
  test-k8s)
    command_string='export KSBH_CI_USE_PREBUILT=false; mise run test-k8s'
    ;;
  test-miri)
    command_string='mise run test-miri'
    ;;
  build-docs)
    command_string='bash scripts/ci/prepare-publish-tooling.sh; bash mise-tasks/build-docs-site; bash mise-tasks/build-docs-site-image; bash mise-tasks/build-helm-repo; bash mise-tasks/build-charts-site-image'
    ;;
  helm-artifacts)
    command_string='bash mise-tasks/lint-helm-chart; bash mise-tasks/package-helm-chart'
    ;;
  *)
    echo "unknown ci job: ${job_name}" >&2
    exit 1
    ;;
esac

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

if [ -z "${CI_BASE_IMAGE:-}" ] && [ -n "${HARBOR_HOST:-}" ]; then
  case "${job_name}" in
    test-k8s|helm-artifacts)
      export CI_BASE_IMAGE="${HARBOR_HOST}/registry/act-rust-kind-playwright:latest"
      ;;
    build-docs|publish-images)
      export CI_BASE_IMAGE="${HARBOR_HOST}/registry/act-rust-wasm-node:latest"
      ;;
    *)
      export CI_BASE_IMAGE="${HARBOR_HOST}/registry/act-rust:latest"
      ;;
  esac
fi

command_string="set -euo pipefail; export RUSTC_WRAPPER=\"\$(command -v sccache)\"; ${command_string}"

exec bash "${repo_root}/scripts/ci/run-in-ci-container.sh" /bin/bash -lc "${command_string}"
