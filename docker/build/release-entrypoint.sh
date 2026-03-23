#!/bin/sh

set -eu

builtin_modules_dir="/usr/lib/ksbh/modules"
runtime_modules_dir="${KSBH__CONFIG_PATHS__MODULES:-/app/modules}"
runtime_config_dir="${KSBH__CONFIG_PATHS__CONFIG:-/app/config/config.yaml}"

mkdir -p "${runtime_modules_dir}"
mkdir -p "$(dirname "${runtime_config_dir}")"

if [ -d "${builtin_modules_dir}" ]; then
  find "${builtin_modules_dir}" -maxdepth 1 -type f -name '*.so' -exec cp -n {} "${runtime_modules_dir}/" \;
fi

exec /app/ksbh "$@"
