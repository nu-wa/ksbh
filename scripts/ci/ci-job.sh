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
    command_string='mise run --skip-tools compile-test-binaries'
    ;;
  test-binary)
    command_string='export KSBH_CI_USE_PREBUILT=false; mise run --skip-tools test-binary'
    ;;
  test-modules)
    command_string='export KSBH_CI_USE_PREBUILT=false; mise run --skip-tools test-modules-smoke; mise run --skip-tools test-unhappy'
    ;;
  test-k8s)
    command_string='export KSBH_CI_USE_PREBUILT=false; mise run --skip-tools test-k8s'
    ;;
  test-miri)
    command_string='mise run --skip-tools test-miri'
    ;;
  build-docs)
    command_string='bash mise-tasks/build-docs-site; bash mise-tasks/build-docs-site-image; bash mise-tasks/build-helm-repo; bash mise-tasks/build-charts-site-image'
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

command_string="set -euo pipefail; ${command_string}"

exec bash "${repo_root}/scripts/ci/run-in-ci-container.sh" /bin/bash -lc "${command_string}"
