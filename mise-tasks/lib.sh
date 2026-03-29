#!/usr/bin/env bash

require_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    echo "docker is not installed or not in PATH" >&2
    exit 1
  fi

  if ! docker info >/dev/null 2>&1; then
    echo "docker daemon is not available" >&2
    exit 1
  fi
}

require_kind() {
  if ! command -v kind >/dev/null 2>&1; then
    echo "kind is not installed or not in PATH" >&2
    exit 1
  fi
}

require_helm() {
  if ! command -v helm >/dev/null 2>&1; then
    echo "helm is not installed or not in PATH" >&2
    exit 1
  fi
}

require_kubectl() {
  if ! command -v kubectl >/dev/null 2>&1; then
    echo "kubectl is not installed or not in PATH" >&2
    exit 1
  fi
}

require_curl() {
  if ! command -v curl >/dev/null 2>&1; then
    echo "curl is not installed or not in PATH" >&2
    exit 1
  fi
}

kind_cluster_name() {
  printf '%s\n' "${KIND_CLUSTER_NAME:-ksbh-local-cluster}"
}

kind_cluster_config() {
  printf '%s\n' "${KIND_CLUSTER_CONFIG:-tests/kind/cluster-ci.yaml}"
}

ensure_kind_cluster() {
  local cluster_name
  local cluster_config
  cluster_name="$(kind_cluster_name)"
  cluster_config="$(kind_cluster_config)"

  if kind get clusters | grep -Fxq "${cluster_name}"; then
    echo "kind cluster '${cluster_name}' already exists"
    return 0
  fi

  kind create cluster --name "${cluster_name}" --config "${cluster_config}"
}

delete_kind_cluster() {
  local cluster_name
  cluster_name="$(kind_cluster_name)"

  if ! kind get clusters | grep -Fxq "${cluster_name}"; then
    echo "kind cluster '${cluster_name}' does not exist"
    return 0
  fi

  kind delete cluster --name "${cluster_name}"
}

release_image_name() {
  printf '%s\n' "${KSBH_RELEASE_IMAGE:-ksbh:release}"
}

release_image_repository() {
  printf '%s\n' "${KSBH_RELEASE_IMAGE_REPOSITORY:-ksbh}"
}

release_image_tag() {
  printf '%s\n' "${KSBH_RELEASE_IMAGE_TAG:-release}"
}

e2e_namespace() {
  printf '%s\n' "${KSBH_E2E_NAMESPACE:-ksbh}"
}

e2e_release_name() {
  printf '%s\n' "${KSBH_E2E_RELEASE_NAME:-ksbh}"
}

resolve_test_binary() {
  local deps_dir="$1"
  local prefix="$2"

  find "${deps_dir}" -maxdepth 1 -type f -name "${prefix}-*" \
    ! -name '*.d' \
    ! -name '*.rlib' \
    ! -name '*.rmeta' \
    | sort \
    | head -n 1
}

wait_for_http_status() {
  local url="$1"
  local expected_status="$2"
  local timeout_secs="${3:-60}"
  local host_header="${4:-}"
  local start_ts
  local now_ts
  local status
  local curl_exit
  local body_file
  local err_file

  start_ts="$(date +%s)"
  body_file="/tmp/ksbh-curl-body.$$"
  err_file="/tmp/ksbh-curl-err.$$"

  while true; do
    curl_exit=0
    if [ -n "${host_header}" ]; then
      status="$(
        curl -k -sS \
          --connect-timeout 2 \
          --max-time 5 \
          -o "${body_file}" \
          -w '%{http_code}' \
          -H "Host: ${host_header}" \
          "${url}" \
          2>"${err_file}"
      )" || curl_exit=$?
    else
      status="$(
        curl -k -sS \
          --connect-timeout 2 \
          --max-time 5 \
          -o "${body_file}" \
          -w '%{http_code}' \
          "${url}" \
          2>"${err_file}"
      )" || curl_exit=$?
    fi

    if [ "${curl_exit}" -eq 0 ] && [ "${status}" = "${expected_status}" ]; then
      rm -f "${body_file}" "${err_file}"
      return 0
    fi

    now_ts="$(date +%s)"
    if [ $((now_ts - start_ts)) -ge "${timeout_secs}" ]; then
      echo "timed out waiting for ${url} to return ${expected_status}, last status was ${status}, curl exit was ${curl_exit}" >&2
      if [ -f "${err_file}" ]; then
        echo "last curl stderr:" >&2
        cat "${err_file}" >&2 || true
      fi
      if [ -f "${body_file}" ]; then
        echo "last response body:" >&2
        cat "${body_file}" >&2 || true
      fi
      rm -f "${body_file}" "${err_file}"
      return 1
    fi

    sleep 1
  done
}
