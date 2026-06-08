#!/usr/bin/env bash
# Per-scenario bench runner.
# Called as: mise run bench:run-scenario <scenario_id>
# Spins up upstream + nginx containers, runs ksbh as a host process, drives
# vegeta load against each proxy, scrapes metrics / RSS, and writes one result
# JSON per (scenario, proxy) to bench/results/${SCENARIO}_${PROXY}.json.
#
# All template / JSON / aggregation logic lives in the ksbh-bench crate. This
# file is the host-side lifecycle wrapper: tool checks, port allocation, cert
# generation, container lifecycle, scrapers, vegeta invocation.
set -euo pipefail

SCENARIO="${1:-}"
if [[ -z "$SCENARIO" ]]; then
  echo "usage: $0 <scenario_id>" >&2
  exit 2
fi

# ---------------------------------------------------------------------------
# All scratch files (cert, configs, vegeta I/O, logs) live under $HOME because
# colima's Docker VM can't bind-mount /tmp cleanly.
# ---------------------------------------------------------------------------
BENCH_TMP="${BENCH_TMP:-$HOME/bench-tmp/$SCENARIO}"
mkdir -p "$BENCH_TMP"
trap 'rm -rf "$BENCH_TMP"' EXIT

# ---------------------------------------------------------------------------
# Tool checks
# ---------------------------------------------------------------------------
for tool in vegeta docker openssl; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "error: '$tool' not found in PATH" >&2
    case "$tool" in
      vegeta) echo "  install with: mise install" >&2 ;;
      docker) echo "  install Docker Desktop (macOS) or dockerd (Linux)" >&2 ;;
      openssl) echo "  install openssl (e.g. 'brew install openssl' or 'apt install openssl')" >&2 ;;
    esac
    exit 3
  fi
done

# ksbh-bench: prefer PATH lookup, but fall back to the prebuilt binary the
# same way KSBH_BIN does below. This keeps `mise run build-rust && mise run
# bench:run-scenario` working without a `cargo install` step.
KSBH_BENCH_BIN="${KSBH_BENCH_BIN:-}"
if [[ -n "$KSBH_BENCH_BIN" ]]; then
  if [[ ! -x "$KSBH_BENCH_BIN" ]]; then
    echo "error: KSBH_BENCH_BIN points to a non-executable: $KSBH_BENCH_BIN" >&2
    exit 3
  fi
elif command -v ksbh-bench >/dev/null 2>&1; then
  KSBH_BENCH_BIN="ksbh-bench"
elif [[ -x "crates/target/debug/ksbh-bench" ]]; then
  KSBH_BENCH_BIN="crates/target/debug/ksbh-bench"
else
  echo "error: ksbh-bench not found" >&2
  echo "  run 'mise run build-rust' first, install it on PATH, or set KSBH_BENCH_BIN=/path/to/ksbh-bench" >&2
  exit 3
fi

# ---------------------------------------------------------------------------
# Scenario descriptor (bench/scenarios/<id>.toml) — read the few fields the
# runner actually needs (duration, rate, connections, body_bytes, tls).
# ---------------------------------------------------------------------------
SCENARIO_TOML="bench/scenarios/${SCENARIO}.toml"
if [[ ! -f "$SCENARIO_TOML" ]]; then
  echo "error: missing scenario descriptor: $SCENARIO_TOML" >&2
  exit 4
fi

scenario_field() {
  local key="$1"
  awk -v k="$key" '
    /^\[scenario\]/ { in_s=1; next }
    /^\[/           { in_s=0; next }
    in_s {
      eq = index($0, "=")
      if (eq == 0) next
      lhs = substr($0, 1, eq - 1)
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", lhs)
      if (lhs != k) next
      val = substr($0, eq + 1)
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", val)
      # Strip a trailing TOML comment. Only treat `#` as a comment when it
      # appears outside the value (e.g. after a space). Quoted strings aren'\''t
      # expected for the fields we read, so this is enough.
      sub(/[[:space:]]+#[^"]*$/, "", val)
      gsub(/"/, "", val)
      print val
      exit
    }
  ' "$SCENARIO_TOML"
}

DURATION_S=$(scenario_field duration_s)
RATE=$(scenario_field rate)
CONNECTIONS=$(scenario_field connections)
TLS=$(scenario_field tls)
BODY_BYTES=$(scenario_field body_bytes)
if [[ -z "$DURATION_S" || -z "$RATE" || -z "$CONNECTIONS" || -z "$TLS" || -z "$BODY_BYTES" ]]; then
  echo "error: scenario $SCENARIO_TOML is missing one of duration_s/rate/connections/tls/body_bytes" >&2
  exit 4
fi

# ---------------------------------------------------------------------------
# Free-port allocator (6 ports: upstream, ksbh http/https/internal/metrics,
# nginx http)
# ---------------------------------------------------------------------------
free_port() {
  python3 -c 'import socket; s=socket.socket(); s.bind(("",0)); print(s.getsockname()[1])'
}
UPSTREAM_PORT=$(free_port)
KSBH_HTTP_PORT=$(free_port)
KSBH_HTTPS_PORT=$(free_port)
KSBH_INTERNAL_PORT=$(free_port)
KSBH_METRICS_PORT=$(free_port)
NGINX_HTTP_PORT=$(free_port)

# ---------------------------------------------------------------------------
# Self-signed cert (valid 1 day, SAN covers bench.local + localhost + 127.0.0.1)
# ---------------------------------------------------------------------------
CERT_PATH="$BENCH_TMP/cert.pem"
KEY_PATH="$BENCH_TMP/key.pem"
openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout "$KEY_PATH" \
  -out    "$CERT_PATH" \
  -days 1 \
  -subj /CN=bench.local \
  -addext "subjectAltName=DNS:bench.local,DNS:localhost,IP:127.0.0.1" \
  >/dev/null 2>&1

# ---------------------------------------------------------------------------
# Render configs (ksbh.yaml, nginx.conf, vegeta target files) via ksbh-bench.
# The vegeta target file embeds {{ proxy_http_port }}, so we render twice —
# once with ksbh's port and once with nginx's. ksbh.yaml and nginx.conf are
# identical between the two calls; we keep the first call's output for those.
# ---------------------------------------------------------------------------
render_to() {
  local out_dir="$1" http_port="$2"
  "$KSBH_BENCH_BIN" render "$SCENARIO_TOML" \
    --out                  "$out_dir" \
    --upstream-port        "$UPSTREAM_PORT" \
    --proxy-http-port      "$http_port" \
    --proxy-https-port     "$KSBH_HTTPS_PORT" \
    --proxy-internal-port  "$KSBH_INTERNAL_PORT" \
    --proxy-metrics-port   "$KSBH_METRICS_PORT" \
    --nginx-http-port      "$NGINX_HTTP_PORT" \
    --cert-path            /etc/bench/cert.pem \
    --key-path             /etc/bench/key.pem \
    --host-cert-path       "$CERT_PATH" \
    --host-key-path        "$KEY_PATH"
}
RENDER_OUT_KSBH="$BENCH_TMP/render-ksbh"
RENDER_OUT_NGINX="$BENCH_TMP/render-nginx"
render_to "$RENDER_OUT_KSBH"  "$KSBH_HTTP_PORT"
render_to "$RENDER_OUT_NGINX" "$NGINX_HTTP_PORT"

KSBH_YAML_RENDERED="$BENCH_TMP/ksbh.yaml"
NGINX_CONF_RENDERED="$BENCH_TMP/nginx.conf"
VEGETA_TARGETS_KSBH="$BENCH_TMP/vegeta-ksbh.targets"
VEGETA_TARGETS_NGINX="$BENCH_TMP/vegeta-nginx.targets"
cp "$RENDER_OUT_KSBH/ksbh.yaml"       "$KSBH_YAML_RENDERED"
cp "$RENDER_OUT_KSBH/nginx.conf"      "$NGINX_CONF_RENDERED"
cp "$RENDER_OUT_KSBH/vegeta.targets"  "$VEGETA_TARGETS_KSBH"
cp "$RENDER_OUT_NGINX/vegeta.targets" "$VEGETA_TARGETS_NGINX"

# ---------------------------------------------------------------------------
# Container names + cleanup trap
# ---------------------------------------------------------------------------
UPSTREAM_NAME="bench-upstream-${SCENARIO}"
NGINX_NAME="bench-nginx-${SCENARIO}"
KSBH_PID=""

cleanup() {
  set +e
  if [[ -n "$KSBH_PID" ]] && kill -0 "$KSBH_PID" 2>/dev/null; then
    kill "$KSBH_PID" 2>/dev/null || true
    for _ in 1 2 3; do
      kill -0 "$KSBH_PID" 2>/dev/null || break
      sleep 1
    done
    if kill -0 "$KSBH_PID" 2>/dev/null; then
      kill -9 "$KSBH_PID" 2>/dev/null || true
    fi
    wait "$KSBH_PID" 2>/dev/null || true
  fi
  for c in "$UPSTREAM_NAME" "$NGINX_NAME"; do
    docker rm -f "$c" >/dev/null 2>&1 || true
  done
}
trap cleanup EXIT

# ---------------------------------------------------------------------------
# Health-check helper
# ---------------------------------------------------------------------------
wait_for() {
  local url="$1" timeout="${2:-30}" name="${3:-service}" host_header="${4:-}"
  local i=0
  while (( i < timeout )); do
    if [[ -n "$host_header" ]]; then
      curl -sf -o /dev/null --max-time 2 -H "$host_header" "$url" && return 0
    else
      curl -sf -o /dev/null --max-time 2 "$url" && return 0
    fi
    sleep 1
    i=$((i + 1))
  done
  echo "error: $name did not become healthy at $url within ${timeout}s" >&2
  if [[ "$name" == "ksbh" ]]; then
    echo "--- ksbh log tail ---" >&2
    tail -20 "$BENCH_TMP/ksbh.log" 2>/dev/null >&2 || true
  elif [[ "$name" == "nginx" ]]; then
    echo "--- nginx container status ---" >&2
    docker ps -a --filter "name=$NGINX_NAME" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}" >&2 || true
    echo "--- nginx log tail ---" >&2
    docker logs --tail 30 "$NGINX_NAME" 2>&1 >&2 || true
  fi
  return 1
}

# ---------------------------------------------------------------------------
# Start upstream
# ---------------------------------------------------------------------------
docker run -d --rm --name "$UPSTREAM_NAME" \
  -p "127.0.0.1:${UPSTREAM_PORT}:8080" \
  bench-upstream:latest >/dev/null
wait_for "http://127.0.0.1:${UPSTREAM_PORT}/healthz" 10 "upstream"

# ---------------------------------------------------------------------------
# Start ksbh (host process — uses the prebuilt binary in crates/target/debug)
# ---------------------------------------------------------------------------
KSBH_BIN="${KSBH_BIN:-crates/target/debug/ksbh}"
if [[ ! -x "$KSBH_BIN" ]]; then
  echo "error: ksbh binary not found or not executable: $KSBH_BIN" >&2
  echo "  run 'mise run build-rust' first, or set KSBH_BIN=/path/to/ksbh" >&2
  exit 5
fi

KSBH__COOKIE_KEY="$(head -c 64 /dev/urandom | base64)" \
KSBH__CONFIG_PATHS__CONFIG="$KSBH_YAML_RENDERED" \
KSBH__LISTEN_ADDRESSES__HTTP="127.0.0.1:${KSBH_HTTP_PORT}" \
KSBH__LISTEN_ADDRESSES__HTTPS="127.0.0.1:${KSBH_HTTPS_PORT}" \
KSBH__LISTEN_ADDRESSES__INTERNAL="127.0.0.1:${KSBH_INTERNAL_PORT}" \
KSBH__LISTEN_ADDRESSES__PROMETHEUS="127.0.0.1:${KSBH_METRICS_PORT}" \
KSBH__TLS__DEFAULT_CERT_FILE="$CERT_PATH" \
KSBH__TLS__DEFAULT_KEY_FILE="$KEY_PATH" \
"$KSBH_BIN" >"$BENCH_TMP/ksbh.log" 2>&1 &
KSBH_PID=$!
wait_for "http://127.0.0.1:${KSBH_INTERNAL_PORT}/healthz" 30 "ksbh"

# ---------------------------------------------------------------------------
# Start nginx
# ---------------------------------------------------------------------------
docker run -d --name "$NGINX_NAME" \
  -p "127.0.0.1:${NGINX_HTTP_PORT}:${NGINX_HTTP_PORT}" \
  -v "$NGINX_CONF_RENDERED:/etc/nginx/nginx.conf:ro" \
  -v "$CERT_PATH:/etc/bench/cert.pem:ro" \
  -v "$KEY_PATH:/etc/bench/key.pem:ro" \
  nginx:1.27 >/dev/null
wait_for "http://127.0.0.1:${NGINX_HTTP_PORT}/__bench_healthz" 60 "nginx"

# ---------------------------------------------------------------------------
# Per-run metadata. git_sha is read by `ksbh-bench analyze` from $BENCH_GIT_SHA;
# machine info is auto-detected by the analyzer.
# ---------------------------------------------------------------------------
GIT_SHA=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
STARTED_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
export BENCH_GIT_SHA="$GIT_SHA"

# ---------------------------------------------------------------------------
# Per-proxy timed run
# ---------------------------------------------------------------------------
mkdir -p bench/results

for PROXY in ksbh nginx; do
  METRICS_FILE="$BENCH_TMP/metrics-${PROXY}.jsonl"
  RSS_FILE="$BENCH_TMP/rss-${PROXY}.txt"
  VEGETA_BIN="$BENCH_TMP/vegeta-${PROXY}.bin"
  VEGETA_REPORT="$BENCH_TMP/report-${PROXY}.json"
  : > "$METRICS_FILE"
  : > "$RSS_FILE"

  METRICS_PID=""
  RSS_PID=""

  # Metrics scraper (ksbh only; nginx has no /metrics endpoint).
  if [[ "$PROXY" == "ksbh" ]]; then
    (
      while true; do
        curl -s --max-time 2 "http://127.0.0.1:${KSBH_METRICS_PORT}/metrics" \
          >> "$METRICS_FILE" 2>/dev/null || true
        printf '\n' >> "$METRICS_FILE"
        sleep 1
      done
    ) &
    METRICS_PID=$!
  fi

  # RSS sampler. ksbh runs as a host process so `ps -o rss=` works directly.
  # nginx runs in a container: on macOS the in-container PID is in the VM's
  # namespace (not visible to host `ps`), and `docker stats` only exposes
  # MemUsage (which includes cache). So we read VmRSS directly from
  # /proc/1/status inside the container — it's a stable KiB integer that
  # matches what `ps -o rss=` reports for a host process, and the analyzer
  # parses each line as u64.
  if [[ "$PROXY" == "ksbh" ]]; then
    TARGET_PID="$KSBH_PID"
    (
      while kill -0 "$TARGET_PID" 2>/dev/null; do
        ps -o rss= -p "$TARGET_PID" 2>/dev/null >> "$RSS_FILE" || true
        sleep 0.5
      done
    ) &
  else
    CONTAINER_NAME="bench-${PROXY}-${SCENARIO}"
    (
      while :; do
        if ! docker inspect "$CONTAINER_NAME" >/dev/null 2>&1; then
          break
        fi
        docker exec "$CONTAINER_NAME" \
          awk '/VmRSS/ {print $2}' /proc/1/status 2>/dev/null >> "$RSS_FILE" || true
        sleep 0.5
      done
    ) &
  fi
  RSS_PID=$!

  # Per-proxy warm-up at low rate (discards JIT/COW effects, ignores cold-start
  # 5xx). Hits the proxy under test so its connection pool is warm.
  if [[ "$PROXY" == "ksbh" ]]; then
    VEGETA_TARGETS="$VEGETA_TARGETS_KSBH"
  else
    VEGETA_TARGETS="$VEGETA_TARGETS_NGINX"
  fi
  vegeta attack \
    -targets="$VEGETA_TARGETS" \
    -rate=100 -duration=5s \
    -timeout=10s -connections="$CONNECTIONS" -max-workers=8 -insecure \
    >/dev/null 2>&1 || true

  vegeta attack \
    -targets="$VEGETA_TARGETS" \
    -output="$VEGETA_BIN" \
    -duration="${DURATION_S}s" \
    -timeout=10s \
    -connections="$CONNECTIONS" \
    -max-workers=8 \
    -insecure \
    -rate="$RATE"
  vegeta report -type=json "$VEGETA_BIN" > "$VEGETA_REPORT"

  # Stop the BG scraper/sampler and wait for them to actually exit.
  [[ -n "$METRICS_PID" ]] && kill "$METRICS_PID" 2>/dev/null || true
  [[ -n "$RSS_PID" ]]    && kill "$RSS_PID"    2>/dev/null || true
  [[ -n "$METRICS_PID" ]] && wait "$METRICS_PID" 2>/dev/null || true
  [[ -n "$RSS_PID" ]]    && wait "$RSS_PID"    2>/dev/null || true

  RESULT_FILE="bench/results/${SCENARIO}_${PROXY}.json"
  "$KSBH_BENCH_BIN" analyze \
    --proxy             "$PROXY" \
    --vegeta-report     "$VEGETA_REPORT" \
    --metrics           "$METRICS_FILE" \
    --rss               "$RSS_FILE" \
    --scenario          "$SCENARIO" \
    --started-at        "$STARTED_AT" \
    --duration-s        "$DURATION_S" \
    --concurrency       "$CONNECTIONS" \
    --tls="$TLS" \
    --body-bytes        "$BODY_BYTES" \
    --method            "GET" \
    --path              "/" \
    --host              "bench.local" \
    --out               "$RESULT_FILE"
done

echo "ok: results in bench/results/${SCENARIO}_{ksbh,nginx}.json"
