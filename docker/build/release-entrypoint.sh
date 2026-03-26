#!/bin/sh

set -eu

builtin_modules_dir="/usr/lib/ksbh/modules"
runtime_modules_dir="${KSBH__CONFIG_PATHS__MODULES:-/app/modules}"
runtime_config_dir="${KSBH__CONFIG_PATHS__CONFIG:-/app/config/config.yaml}"
default_tls_cert_file="${KSBH__TLS__DEFAULT_CERT_FILE:-/app/config/default-tls.crt}"
default_tls_key_file="${KSBH__TLS__DEFAULT_KEY_FILE:-/app/config/default-tls.key}"

mkdir -p "${runtime_modules_dir}"
mkdir -p "$(dirname "${runtime_config_dir}")"
mkdir -p "$(dirname "${default_tls_cert_file}")"
mkdir -p "$(dirname "${default_tls_key_file}")"

if [ -d "${builtin_modules_dir}" ]; then
  find "${builtin_modules_dir}" -maxdepth 1 -type f -name '*.so' -exec cp -n {} "${runtime_modules_dir}/" \;
fi

if [ ! -s "${default_tls_cert_file}" ] || [ ! -s "${default_tls_key_file}" ]; then
  echo "Generating fallback TLS certificate at ${default_tls_cert_file}"
  openssl req \
    -x509 \
    -nodes \
    -newkey rsa:2048 \
    -keyout "${default_tls_key_file}" \
    -out "${default_tls_cert_file}" \
    -days 3650 \
    -subj "/CN=ksbh.local"
  chmod 600 "${default_tls_key_file}" || true
fi

export KSBH__TLS__DEFAULT_CERT_FILE="${default_tls_cert_file}"
export KSBH__TLS__DEFAULT_KEY_FILE="${default_tls_key_file}"

exec /app/ksbh "$@"
