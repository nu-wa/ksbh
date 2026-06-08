//! Aggregate a vegeta JSON report + ksbh metrics scrape + RSS samples into
//! a single Result JSON matching the schema written under `bench/history/`.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct AnalyzeArgs {
    pub proxy: String,
    pub vegeta_report: String,
    pub metrics: String,
    pub rss: String,
    pub scenario: String,
    pub proxy_version: String,
    pub started_at: String,
    pub duration_s: u32,
    pub concurrency: u32,
    pub tls: bool,
    pub body_bytes: u32,
    pub method: String,
    pub path: String,
    pub host: String,
    pub cpu_avg: Option<f64>,
    pub out: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResultDoc {
    pub schema_version: String,
    pub scenario: String,
    pub proxy: String,
    pub git_sha: String,
    pub machine: MachineInfo,
    pub started_at: String,
    pub duration_s: u32,
    pub concurrency: u32,
    pub tls: bool,
    pub workload: Workload,
    pub results: Results,
    pub proxy_peak_rss_kb: u64,
    pub proxy_metrics_snapshot: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MachineInfo {
    pub cpus: u32,
    pub model: String,
    pub kernel: String,
    pub hostname: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Workload {
    pub method: String,
    pub path: String,
    pub host: String,
    pub body_bytes: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Results {
    pub rps: f64,
    pub p50_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
    pub errors: Errors,
    pub bytes_proxied: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Errors {
    #[serde(rename = "5xx")]
    pub s5xx: u64,
    #[serde(rename = "4xx")]
    pub s4xx: u64,
    pub connect_failed: u64,
    pub read_timeout: u64,
}

pub fn analyze(args: &AnalyzeArgs) -> Result<ResultDoc> {
    let vegeta: serde_json::Value = read_json(&args.vegeta_report)?;
    let metrics_snapshot = read_last_metrics_scrape(&args.metrics)?;
    let rss_kb = read_peak_rss_kb(&args.rss).unwrap_or(0);

    let results = analyze_vegeta(&vegeta)?;
    let machine = detect_machine();

    let doc = ResultDoc {
        schema_version: "1".to_string(),
        scenario: args.scenario.clone(),
        proxy: args.proxy.clone(),
        git_sha: std::env::var("BENCH_GIT_SHA").unwrap_or_default(),
        machine,
        started_at: args.started_at.clone(),
        duration_s: args.duration_s,
        concurrency: args.concurrency,
        tls: args.tls,
        workload: Workload {
            method: args.method.clone(),
            path: args.path.clone(),
            host: args.host.clone(),
            body_bytes: args.body_bytes,
        },
        results,
        proxy_peak_rss_kb: rss_kb,
        proxy_metrics_snapshot: metrics_snapshot,
    };

    let json = serde_json::to_string_pretty(&doc)?;
    if let Some(parent) = Path::new(&args.out).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating output dir {}", parent.display()))?;
        }
    }
    fs::write(&args.out, json)
        .with_context(|| format!("writing result to {}", args.out))?;

    Ok(doc)
}

/// Extract the per-request metrics from a vegeta JSON report.
pub fn analyze_vegeta(vegeta: &serde_json::Value) -> Result<Results> {
    let rps = vegeta
        .get("rate")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let lat = vegeta.get("latencies").cloned().unwrap_or_default();
    let ns_to_ms = |k: &str| -> f64 {
        lat.get(k)
            .and_then(|v| v.as_f64())
            .map(|n| n / 1_000_000.0)
            .unwrap_or(0.0)
    };
    let p50_ms = ns_to_ms("50th");
    let p99_ms = ns_to_ms("99th");
    let max_ms = ns_to_ms("max");

    let bytes_in = vegeta
        .pointer("/bytes_in/total")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let bytes_out = vegeta
        .pointer("/bytes_out/total")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let bytes_proxied = bytes_in + bytes_out;

    let errors = count_errors(vegeta);

    Ok(Results {
        rps,
        p50_ms,
        p99_ms,
        max_ms,
        errors,
        bytes_proxied,
    })
}

fn count_errors(vegeta: &serde_json::Value) -> Errors {
    let mut e = Errors::default();

    if let Some(codes) = vegeta.get("status_codes").and_then(|v| v.as_object()) {
        for (code, count) in codes {
            let n = count.as_u64().unwrap_or(0);
            if code.starts_with('5') {
                e.s5xx += n;
            } else if code.starts_with('4') {
                e.s4xx += n;
            }
        }
    }

    if let Some(errs) = vegeta.get("errors").and_then(|v| v.as_array()) {
        for msg in errs {
            let s = msg.as_str().unwrap_or("");
            if s.contains("no such host") || s.contains("connection refused") {
                e.connect_failed += 1;
            } else if s.contains("context deadline exceeded") {
                e.read_timeout += 1;
            }
        }
    }

    e
}

fn read_json(path: &str) -> Result<serde_json::Value> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path))?;
    serde_json::from_str(&raw).with_context(|| format!("parsing {} as JSON", path))
}

/// Read all lines from a JSONL file and return the last line that parses as
/// a JSON object (i.e. the last full scrape).
fn read_last_metrics_scrape(path: &str) -> Result<String> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("reading metrics file {}", path))?;
    let mut last = String::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if serde_json::from_str::<serde_json::Value>(line).is_ok() {
            last = line.to_string();
        }
    }
    Ok(last)
}

fn read_peak_rss_kb(path: &str) -> Result<u64> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("reading rss file {}", path))?;
    let mut peak: u64 = 0;
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(v) = line.parse::<u64>() {
            if v > peak {
                peak = v;
            }
        }
    }
    Ok(peak)
}

fn detect_machine() -> MachineInfo {
    let cpus = detect_cpus();
    let model = detect_model();
    let kernel = run_cmd("uname", &["-r"]).unwrap_or_default();
    let hostname = run_cmd("hostname", &[]).unwrap_or_default();
    MachineInfo {
        cpus,
        model,
        kernel,
        hostname,
    }
}

fn detect_cpus() -> u32 {
    if let Ok(s) = run_cmd("nproc", &[]) {
        if let Ok(n) = s.trim().parse::<u32>() {
            return n;
        }
    }
    if let Ok(s) = run_cmd("sysctl", &["-n", "hw.ncpu"]) {
        if let Ok(n) = s.trim().parse::<u32>() {
            return n;
        }
    }
    1
}

fn detect_model() -> String {
    if let Ok(s) = run_cmd("sysctl", &["-n", "machdep.cpu.brand_string"]) {
        let t = s.trim().to_string();
        if !t.is_empty() {
            return t;
        }
    }
    if let Ok(raw) = fs::read_to_string("/proc/cpuinfo") {
        for line in raw.lines() {
            if let Some(rest) = line.strip_prefix("model name\t: ") {
                return rest.to_string();
            }
        }
    }
    String::new()
}

fn run_cmd(prog: &str, args: &[&str]) -> Result<String> {
    let out = std::process::Command::new(prog)
        .args(args)
        .output()
        .with_context(|| format!("spawning {}", prog))?;
    if !out.status.success() {
        anyhow::bail!("{} exited with {:?}", prog, out.status.code());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
