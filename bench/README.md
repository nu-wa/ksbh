# bench

Local bench harness for ksbh. Compares ksbh against nginx 1.27 across five speed scenarios and four reverse-proxy robustness scenarios, and records ksbh-vs-ksbh history so a contributor can ask "did my PR regress ksbh?" by diffing a new run against an old one. History is stored in `bench/history/`, and an inline-SVG trend page is emitted to `docs/static/bench/index.html`. CI integration is deferred; see "Future: CI" at the bottom.

All bench tooling lives in the `ksbh-bench` crate at `crates/ksbh-bench/`. The Python that used to live under `bench/aggregate/`, `bench/compare/`, and `bench/trend/` is gone — those operations are now Rust subcommands. The runner (`bench/run-scenario.sh`) is a thin bash lifecycle wrapper that calls the crate.

## Prerequisites

- `mise` with shims on PATH (or use `mise run` / `mise exec` consistently)
- Docker daemon (Docker Desktop on macOS, `dockerd` on Linux)
- `openssl` on PATH
- `cargo` (Rust 1.92.0, pinned via `mise.toml`)

`mise install` installs vegeta v12.12.0. No Python tooling is required. The upstream Docker image is `python:3.12-alpine` and is self-contained.

## Quick start

```bash
mise install
mise run bench:upstream:build
cargo build                # produces crates/target/debug/ksbh and crates/target/debug/ksbh-bench
mise run bench:ksbh:build
mise run bench:all         # ~10-15 min, all 9 scenarios
mise run bench:commit      # stage history + trend page for the current commit
git diff --stat            # review, then git commit -m 'bench: ...' && git push
```

To run a single scenario instead of the full suite:

```bash
mise run bench:run-scenario speed_small_c1
```

## Tasks reference

| Task | What it does |
|------|--------------|
| `bench:install-tools` | Install vegeta via mise (ksbh-bench is built locally via `build-rust`) |
| `bench:upstream:build` | Build the `bench-upstream` Docker image |
| `bench:ksbh:build` | Build the `bench-ksbh` Docker image from the prebuilt binary |
| `bench:run-scenario` | Run a single scenario against ksbh and nginx (`bench:run-scenario <id>`) |
| `bench:run-all` | Run all 9 scenarios (speed + robustness) against ksbh and nginx |
| `bench:aggregate` | `ksbh-bench aggregate` — produce `bench/report/bench.md` (and a gate exit code) |
| `bench:compare` | `ksbh-bench compare` — diff two bench runs (`bench:compare <old_sha> <new_sha>`) |
| `bench:trend` | `ksbh-bench trend` — regenerate `docs/static/bench/index.html` from `bench/history/` |
| `bench:gate` | Regression check vs parent commit (exits 1 on REGRESSION) |
| `bench:commit` | Stage bench history + trend page for the current commit |
| `bench:all` | Full local bench: install, build, run ksbh+nginx, aggregate, trend |
| `bench:lint` | Syntax-check `bench/run-scenario.sh` and verify `ksbh-bench` builds |
| `bench:smoke:upstream` | Build the `bench-upstream` image and exercise its endpoints |
| `bench:smoke:render` | Render a scenario via `ksbh-bench render` and confirm the three outputs exist |
| `bench:smoke:synthetic` | Run `aggregate` / `compare` / `trend` on synthetic data |
| `bench:smoke` | Full smoke test: lint, upstream, render, synthetic data |

## Architecture: where the bench lives

```
crates/ksbh-bench/                  # the bench tooling (one binary, four subcommands)
├── src/
│   ├── render.rs                   # scenario TOML → ksbh.yaml + nginx.conf + vegeta.targets
│   ├── analyze.rs                  # vegeta JSON + scrapes + RSS → per-scenario Result JSON
│   ├── aggregate.rs                # Result JSONs → bench.md (Markdown) or stitched history JSON
│   ├── compare.rs                  # two history snapshots → diff table (markdown / json / gate)
│   ├── trend.rs                    # history/*.json → inline-SVG HTML
│   ├── scenario.rs                 # Scenario, ScenarioMeta, VegetaSpec, NginxOverrides types
│   └── template.rs                 # tiny `{{ var }}` substitution helper
└── src/bin/ksbh-bench.rs           # clap CLI: render | analyze | aggregate | compare | trend

bench/
├── scenarios/                      # one TOML per scenario (id, duration_s, rate, connections,
│                                   #   tls, body_bytes, vegeta targets, nginx overrides)
├── run-scenario.sh                 # bash lifecycle wrapper: tool checks, port allocation,
│                                   #   cert gen, container lifecycle, scrapers, vegeta
├── results/                        # per-(scenario,proxy) result JSONs (schema_version: 1)
├── history/                        # committed per-commit stitched JSON snapshots
├── report/                         # aggregate output (bench.md)
├── upstream/                       # Python echo server (intentionally Python; container-only)
├── Dockerfile.upstream             # upstream image
├── Dockerfile.ksbh                 # ksbh image (used by the k8s test pipeline, not bench:v1)
└── README.md                       # this file
```

Per-scenario ksbh ingress YAMLs, nginx configs, and vegeta targets are no longer checked in — they are generated by `ksbh-bench render` at the start of each run from the scenario TOML.

## Scenarios

### Speed (5)

| ID | Workload | Tool | Duration | Headline metric |
|----|----------|------|----------|-----------------|
| `speed_small_c1` | 200 B body, 1 conn, 1 thread | vegeta rate=inf | 30s | RPS |
| `speed_small_c100` | 200 B body, 100 conns | vegeta rate=inf | 30s | RPS |
| `speed_large_c50` | 64 KiB body, 50 conns | vegeta rate=inf | 30s | RPS, p99 |
| `speed_tls_h2` | TLS, h2 prior knowledge, 50 conns | vegeta rate=inf | 30s | RPS, p99 |
| `speed_const_50krps` | vegeta constant 50,000 RPS, 50 conns | vegeta | 30s | RPS (lowest variance) |

`speed_const_50krps` is the headline number on the trend chart; it has the lowest cross-run variance. Upstream is plaintext in v1 for both proxies.

### Robustness (4)

| ID | Adversarial input | Tool | Duration | Headline metric |
|----|-------------------|------|----------|-----------------|
| `robust_slow_loris` | 100 conns sending request line at 1 B/s up to 4 KiB header | vegeta | 30s | conns closed cleanly, peak RSS |
| `robust_upstream_kill` | 5,000 RPS load, `POST /__ctl/shutdown` at T=5s, restart at T=10s, run to T=30s | vegeta | 30s | 502 rate, recovery latency |
| `robust_h2_streams` | 1 h2 client, 1000 multiplexed streams, 1 hits `/__ctl/sleep?ms=2000` | vegeta | 30s | p99 stream latency, head-of-line blocking |
| `robust_malformed` | Bad `Content-Length` (CL > body), illegal chars in request line, oversized headers (>1 MiB) | vegeta | 30s | 4xx rate, zero false-200s, peak RSS |

The four robustness scenarios are driven by vegeta for v1; a per-scenario adversarial driver (slowloris.py, h2 client, malformed-request sender) is not included.

## Output and history

- **Raw results** — one JSON per (scenario, proxy) in `bench/results/`, `schema_version: 1`. The aggregator and diff tools both consume this format.
- **Markdown report** — `mise run bench:aggregate` produces `bench/report/bench.md`, a side-by-side table of ksbh vs nginx for the current run.
- **Committed history** — `mise run bench:commit` stages `bench/history/<sha>.json` and `docs/static/bench/index.html` for the current commit. Review, then `git commit` and push when ready.
- **Diff two runs** — `mise run bench:compare <old_sha> <new_sha>` prints a verdict table (REGRESSION / IMPROVEMENT / NEUTRAL) for every (scenario, metric).
- **Trend page** — `mise run bench:trend` regenerates the SVG trend page from `bench/history/`. Open `docs/static/bench/index.html` in a browser.
- **First run** — there is no parent history, so `mise run bench:gate` is a no-op (exit 0, clear warning). Verdicts appear from the second run onward.

## Comparing two runs

```bash
mise run bench:compare <old_sha> <new_sha>
```

Default thresholds are class-dependent (set in `crates/ksbh-bench/src/compare.rs`):

| Class | RPS | p50 / p99 | RSS | p99 floor |
|-------|-----|-----------|-----|-----------|
| `Speed` | 5% | 10% | 20% | 0.05 ms |
| `SpeedTlsH2` | 15% (absorbs rustls-vs-OpenSSL gap) | 10% | 20% | 0.05 ms |
| `Robust` | 25% | n/a (no gate) | 20% | n/a |

`--format=gate` makes the binary exit non-zero on any REGRESSION, suitable for CI.

## Adding a new scenario

- Create `bench/scenarios/<id>.toml`. The shape is:
  ```toml
  [scenario]
  id          = "speed_medium_c20"
  duration_s  = 30
  rate        = 0              # 0 = saturate; non-zero = constant RPS
  connections = 20
  tls         = false
  body_bytes  = 1024

  [vegeta]
  targets = """
  GET http://{{ proxy_host }}:{{ proxy_http_port }}/
  """

  [nginx]                      # all keys optional; defaults are reasonable
  proxy_buffer_size = "16k"
  proxy_buffers     = "8 16k"
  ```
- The runner reads `[scenario]` fields directly from the TOML (duration, rate, connections, tls, body_bytes). nginx overrides flow through `ksbh-bench render` into the rendered `nginx.conf`.
- Add the ID to the `bench:run-all` loop in `mise.toml`.
- Document the scenario in the "Scenarios" section of this README.

## Asymmetries vs nginx

ksbh and nginx 1.27 are not a 1:1 match. The bench controls for the things that matter most and surfaces the rest:

- **TLS implementations differ.** ksbh uses rustls; nginx uses OpenSSL. p99 on the TLS path (`speed_tls_h2`) is not directly comparable. Treat that scenario as "both proxies on TLS" rather than "ksbh vs nginx TLS". The compare-class threshold for `SpeedTlsH2` widens to 15% to absorb the structural gap.
- **Tunable knobs are not 1:1.** ksbh's `keepalive_timeout`, `large_client_header_buffers`, and worker count do not have exact nginx equivalents. The defaults are chosen to be reasonable for both, not to be byte-identical. (See `BUGS.md` §4 for the gaps a contributor could close in ksbh-in-itself.)
- **Worker model.** ksbh runs a single tokio runtime; nginx here is `worker_processes 1` to match.
- **Plaintext upstream.** v1 configures both proxies to talk plaintext to the upstream, so the proxy side of the path is what's being measured, not TLS to the origin.

If a delta looks like tuning rather than architecture, the configs are the place to start.

## Risks and gotchas

- **Docker daemon required.** On Linux without `dockerd`, the bench cannot run.
- **Local runner is noisy.** Laptops and shared hosts introduce variance. `speed_const_50krps` is the most resilient scenario (constant-RPS, not rate=inf). For trustworthy deltas, run the same scenario three times and take the median.
- **History grows over time.** Roughly 10-30 small files per month. There is no automated cleanup in v1.
- **Profiling features must be off.** `pyroscope` and `jemalloc_pprof` must be off in the prebuilt binary or the bench numbers will be skewed. See `BUGS.md` §5.
- **Robustness scenarios are data-only.** v1 produces measurements for the four robustness scenarios, not pass/fail verdicts. The per-scenario adversarial driver (slowloris.py, h2 client, malformed-request sender) is out of scope; vegeta drives all four today.

## Future: CI

The bench dir is designed so that a future CI job can drop in cleanly. `bench/run-scenario.sh` already reads the `KSBH_IMAGE` env var, defaulting to `bench-ksbh:latest` locally; a CI job can set it to a published `ksbh-release:${sha}` from Harbor. `bench:commit` is wired to use `FORGEJO_TOKEN` for the bot commit. `bench:gate` is the regression check. Adding CI is roughly 50 lines of workflow YAML plus a `FORGEJO_TOKEN` secret. See the "Future: CI integration" section of `/Users/smspl/.claude/plans/yo-bitch-search-online-joyful-charm.md` for the design notes.
