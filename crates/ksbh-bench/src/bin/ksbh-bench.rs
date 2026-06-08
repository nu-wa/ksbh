//! ksbh-bench CLI: render, analyze, compare, trend, run-scenario.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

use ksbh_bench::aggregate;
use ksbh_bench::analyze::{self, AnalyzeArgs};
use ksbh_bench::compare::{self, CompareArgs, Format};
use ksbh_bench::render::{self, RenderCtx};
use ksbh_bench::runner::{self, Mode, RunConfig};
use ksbh_bench::scenario::Scenario;
use ksbh_bench::trend;

#[derive(Parser)]
#[command(
    name = "ksbh-bench",
    about = "Benchmark tooling: render scenarios, analyze results, compare runs, trend history."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Aggregate bench/results/*.json into a Markdown report or stitched JSON.
    Aggregate {
        #[arg(long, value_name = "DIR")] r#in: PathBuf,
        #[arg(long)] out: PathBuf,
    },
    /// Render a scenario TOML into ksbh.yaml, nginx.conf, and vegeta.targets.
    Render {
        scenario: PathBuf,
        #[arg(long)] out: PathBuf,
        #[arg(long)] upstream_port: u16,
        #[arg(long)] proxy_http_port: u16,
        #[arg(long)] proxy_https_port: u16,
        #[arg(long)] proxy_internal_port: u16,
        #[arg(long)] proxy_metrics_port: u16,
        #[arg(long)] nginx_http_port: u16,
        #[arg(long)] cert_path: String,
        #[arg(long)] key_path: String,
        #[arg(long)] host_cert_path: String,
        #[arg(long)] host_key_path: String,
        #[arg(long, default_value = "bench.local")] proxy_host: String,
    },
    /// Aggregate a vegeta report + metrics + RSS into a single Result JSON.
    Analyze {
        #[arg(long)] proxy: String,
        #[arg(long)] vegeta_report: PathBuf,
        #[arg(long)] metrics: PathBuf,
        #[arg(long)] rss: PathBuf,
        #[arg(long)] scenario: String,
        #[arg(long, default_value = "")] proxy_version: String,
        #[arg(long)] started_at: String,
        #[arg(long)] duration_s: u32,
        #[arg(long)] concurrency: u32,
        #[arg(long, action = clap::ArgAction::Set)] tls: bool,
        #[arg(long)] body_bytes: u32,
        #[arg(long, default_value = "GET")] method: String,
        #[arg(long, default_value = "/")] path: String,
        #[arg(long, default_value = "bench.local")] host: String,
        #[arg(long)] cpu_avg: Option<f64>,
        #[arg(long)] out: PathBuf,
    },
    /// Compare two history snapshots.
    Compare {
        #[arg(long)] history: PathBuf,
        #[arg(long)] old: String,
        #[arg(long)] new: String,
        #[arg(long, value_enum, default_value_t = OutputFmt::Markdown)]
        format: OutputFmt,
    },
    /// Render a self-contained HTML trend page from history/*.json.
    Trend {
        #[arg(long)] history: PathBuf,
        #[arg(long)] out: PathBuf,
    },
    /// Run a full benchmark scenario end-to-end (containers, vegeta, analysis).
    RunScenario {
        /// Scenario name, e.g. `speed_small_c1` (looked up under bench/scenarios/).
        scenario: String,

        /// Proxy mode. Defaults to ksbh-vs-nginx.
        #[arg(long, value_enum, default_value_t = CliMode::KsbhVsNginx)]
        mode: CliMode,

        /// Results output directory. Defaults to bench/results.
        #[arg(long, default_value = "bench/results")]
        out: PathBuf,

        /// Override ksbh binary path (auto-discovered if not set).
        #[arg(long)]
        ksbh_bin: Option<PathBuf>,

        /// Override vegeta binary path (defaults to `vegeta` on PATH).
        #[arg(long)]
        vegeta_bin: Option<PathBuf>,

        /// Path to module cdylib (required for ksbh-module mode).
        #[arg(long)]
        module_lib: Option<PathBuf>,
    },
}

#[derive(Copy, Clone, ValueEnum)]
enum OutputFmt {
    Markdown,
    Json,
    Gate,
}

#[derive(Copy, Clone, ValueEnum)]
enum CliMode {
    KsbhOnly,
    KsbhVsNginx,
    KsbhModule,
}

impl From<CliMode> for Mode {
    fn from(v: CliMode) -> Self {
        match v {
            CliMode::KsbhOnly => Mode::KsbhOnly,
            CliMode::KsbhVsNginx => Mode::KsbhVsNginx,
            CliMode::KsbhModule => Mode::KsbhModule,
        }
    }
}

impl From<OutputFmt> for Format {
    fn from(v: OutputFmt) -> Self {
        match v {
            OutputFmt::Markdown => Format::Markdown,
            OutputFmt::Json => Format::Json,
            OutputFmt::Gate => Format::Gate,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Aggregate { r#in, out } => {
            let args = aggregate::AggregateArgs {
                in_dir: r#in.to_string_lossy().to_string(),
                out: out.to_string_lossy().to_string(),
            };
            let code = aggregate::run(&args)?;
            std::process::exit(code);
        }
        Cmd::Render {
            scenario,
            out,
            upstream_port,
            proxy_http_port,
            proxy_https_port,
            proxy_internal_port,
            proxy_metrics_port,
            nginx_http_port,
            cert_path,
            key_path,
            host_cert_path,
            host_key_path,
            proxy_host,
        } => {
            let raw = std::fs::read_to_string(&scenario)
                .with_context(|| format!("reading scenario {}", scenario.display()))?;
            let scn: Scenario = toml::from_str(&raw)
                .with_context(|| format!("parsing scenario {}", scenario.display()))?;
            let ctx = RenderCtx {
                proxy_host,
                upstream_port,
                proxy_http_port,
                proxy_https_port,
                proxy_internal_port,
                proxy_metrics_port,
                nginx_http_port,
                cert_path,
                key_path,
                host_cert_path,
                host_key_path,
            };
            let r = render::render_scenario(&scn, &ctx)?;
            std::fs::create_dir_all(&out)
                .with_context(|| format!("creating out dir {}", out.display()))?;
            std::fs::write(out.join("ksbh.yaml"), r.ksbh_yaml)?;
            std::fs::write(out.join("nginx.conf"), r.nginx_conf)?;
            std::fs::write(out.join("vegeta.targets"), r.vegeta_targets)?;
            Ok(())
        }
        Cmd::Analyze {
            proxy,
            vegeta_report,
            metrics,
            rss,
            scenario,
            proxy_version,
            started_at,
            duration_s,
            concurrency,
            tls,
            body_bytes,
            method,
            path,
            host,
            cpu_avg,
            out,
        } => {
            let args = AnalyzeArgs {
                proxy,
                vegeta_report: vegeta_report.to_string_lossy().to_string(),
                metrics: metrics.to_string_lossy().to_string(),
                rss: rss.to_string_lossy().to_string(),
                scenario,
                proxy_version,
                started_at,
                duration_s,
                concurrency,
                tls,
                body_bytes,
                method,
                path,
                host,
                cpu_avg,
                out: out.to_string_lossy().to_string(),
            };
            let _ = proxy_version; // currently unused; reserved for future banner
            analyze::analyze(&args)?;
            Ok(())
        }
        Cmd::Compare {
            history,
            old,
            new,
            format,
        } => {
            let args = CompareArgs {
                history: history.to_string_lossy().to_string(),
                old,
                new,
                format: format.into(),
            };
            let code = compare::run(&args)?;
            std::process::exit(code);
        }
        Cmd::Trend { history, out } => {
            let history = history.to_string_lossy().to_string();
            let out = out.to_string_lossy().to_string();
            trend::run(&history, &out)?;
            Ok(())
        }
        Cmd::RunScenario {
            scenario,
            mode,
            out,
            ksbh_bin,
            vegeta_bin,
            module_lib,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let config = RunConfig {
                    mode: mode.into(),
                    out_dir: out,
                    ksbh_bin,
                    vegeta_bin,
                    module_lib,
                };
                let output = runner::run(&scenario, config).await?;
                for r in &output.results {
                    eprintln!(
                        "  {}/{} → {}",
                        output.scenario_id,
                        r.proxy,
                        r.result_path.display()
                    );
                }
                Ok(())
            })
        }
    }
}
