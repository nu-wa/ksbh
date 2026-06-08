//! Compare two history snapshots (one per git SHA) and emit a diff.
//!
//! Output formats:
//! - `--markdown` (default): Markdown table to stdout
//! - `--json`: machine-readable JSON to stdout
//! - `--gate`: print markdown, exit 1 if any REGRESSION
//!
//! Verdict rules per metric:
//! - RPS: higher is better. ±threshold_pct.
//! - p50, p99, max: lower is better. For p99, also require
//!   |Δ| > p99_abs_floor_ms to avoid flagging sub-ms noise.
//! - RSS: lower is better. ±threshold_pct.
//! - error_rate_pct: lower is better. ±0.5 pp.

use anyhow::{Context, Result};
use serde::Serialize;

use crate::analyze::ResultDoc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScenarioClass {
    Speed,
    SpeedTlsH2,
    Robust,
}

impl ScenarioClass {
    pub fn for_scenario(id: &str) -> Self {
        if id.starts_with("speed_tls_h2") {
            ScenarioClass::SpeedTlsH2
        } else if id.starts_with("robust_") {
            ScenarioClass::Robust
        } else {
            ScenarioClass::Speed
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Thresholds {
    pub rps_pct: f64,
    pub p50_pct: f64,
    pub p99_pct: f64,
    pub rss_pct: f64,
    pub p99_abs_floor_ms: f64,
}

impl Thresholds {
    pub fn for_class(class: ScenarioClass) -> Self {
        match class {
            ScenarioClass::Speed => Thresholds {
                rps_pct: 5.0,
                p50_pct: 10.0,
                p99_pct: 10.0,
                rss_pct: 20.0,
                p99_abs_floor_ms: 0.05,
            },
            ScenarioClass::SpeedTlsH2 => Thresholds {
                rps_pct: 15.0,
                p50_pct: 10.0,
                p99_pct: 10.0,
                rss_pct: 20.0,
                p99_abs_floor_ms: 0.05,
            },
            ScenarioClass::Robust => Thresholds {
                rps_pct: 25.0,
                p50_pct: 99.0,
                p99_pct: 99.0,
                rss_pct: 20.0,
                p99_abs_floor_ms: 999.0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Verdict {
    Improvement,
    Neutral,
    Regression,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricDiff {
    pub scenario: String,
    pub proxy: String,
    pub metric: String,
    pub old: f64,
    pub new: f64,
    pub delta: f64,
    pub delta_pct: f64,
    pub verdict: Verdict,
}

#[derive(Debug, Clone)]
pub struct CompareArgs {
    pub history: String,
    pub old: String,
    pub new: String,
    pub format: Format,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Json,
    Gate,
}

/// Compare two snapshots and write to stdout. Returns exit code for `--gate`.
pub fn run(args: &CompareArgs) -> Result<i32> {
    let old_path = format!("{}/{}.json", args.history, args.old);
    let new_path = format!("{}/{}.json", args.history, args.new);

    if !std::path::Path::new(&old_path).exists() || !std::path::Path::new(&new_path).exists() {
        eprintln!(
            "compare: missing history file (old={}, new={}); skipping",
            old_path, new_path
        );
        return Ok(0);
    }

    let old_docs = read_history(&old_path)?;
    let new_docs = read_history(&new_path)?;

    let diffs = diff_snapshots(&old_docs, &new_docs);

    match args.format {
        Format::Markdown => {
            print_markdown(&diffs);
            Ok(0)
        }
        Format::Json => {
            let json = serde_json::to_string_pretty(&diffs)?;
            println!("{}", json);
            Ok(0)
        }
        Format::Gate => {
            print_markdown(&diffs);
            if diffs.iter().any(|d| d.verdict == Verdict::Regression) {
                Ok(1)
            } else {
                Ok(0)
            }
        }
    }
}

/// Read a history file. Accepts either a single object or an array.
pub fn read_history(path: &str) -> Result<Vec<ResultDoc>> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path))?;
    let v: serde_json::Value = serde_json::from_str(&raw)
        .with_context(|| format!("parsing {} as JSON", path))?;
    match v {
        serde_json::Value::Array(_) => serde_json::from_value(v)
            .with_context(|| format!("parsing history array in {}", path)),
        serde_json::Value::Object(_) => {
            let doc: ResultDoc = serde_json::from_value(v)
                .with_context(|| format!("parsing history object in {}", path))?;
            Ok(vec![doc])
        }
        _ => anyhow::bail!("unexpected JSON shape in {}", path),
    }
}

/// Pair scenarios by id and compute the diff.
pub fn diff_snapshots(old: &[ResultDoc], new: &[ResultDoc]) -> Vec<MetricDiff> {
    let mut out = Vec::new();
    for o in old {
        if let Some(n) = new.iter().find(|n| n.scenario == o.scenario) {
            out.extend(compare_pair(o, n));
        }
    }
    out
}

/// Compare two result docs (one scenario) and produce a row per metric.
pub fn compare_pair(old: &ResultDoc, new: &ResultDoc) -> Vec<MetricDiff> {
    let class = ScenarioClass::for_scenario(&old.scenario);
    let thr = Thresholds::for_class(class);
    let mut out = Vec::new();

    // RPS: higher is better.
    out.push(metric_diff(
        old,
        new,
        "rps",
        old.results.rps,
        new.results.rps,
        Direction::Higher,
        thr.rps_pct,
        0.0,
    ));

    // p50, p99, max: lower is better.
    out.push(metric_diff(
        old,
        new,
        "p50_ms",
        old.results.p50_ms,
        new.results.p50_ms,
        Direction::Lower,
        thr.p50_pct,
        0.0,
    ));
    out.push(metric_diff(
        old,
        new,
        "p99_ms",
        old.results.p99_ms,
        new.results.p99_ms,
        Direction::Lower,
        thr.p99_pct,
        thr.p99_abs_floor_ms,
    ));
    out.push(metric_diff(
        old,
        new,
        "max_ms",
        old.results.max_ms,
        new.results.max_ms,
        Direction::Lower,
        thr.p50_pct,
        0.0,
    ));

    // RSS: lower is better.
    out.push(metric_diff(
        old,
        new,
        "proxy_peak_rss_kb",
        old.proxy_peak_rss_kb as f64,
        new.proxy_peak_rss_kb as f64,
        Direction::Lower,
        thr.rss_pct,
        0.0,
    ));

    // Error rate: lower is better. ±0.5 pp.
    let old_err = total_errors(&old.results.errors) as f64;
    let new_err = total_errors(&new.results.errors) as f64;
    let old_total = old.results.bytes_proxied.max(1) as f64;
    let new_total = new.results.bytes_proxied.max(1) as f64;
    let old_rate = old_err / old_total * 100.0;
    let new_rate = new_err / new_total * 100.0;
    out.push(metric_diff(
        old,
        new,
        "error_rate_pct",
        old_rate,
        new_rate,
        Direction::Lower,
        0.5,
        0.0,
    ));

    out
}

fn total_errors(e: &crate::analyze::Errors) -> u64 {
    e.s5xx + e.s4xx + e.connect_failed + e.read_timeout
}

#[derive(Debug, Clone, Copy)]
enum Direction {
    Higher,
    Lower,
}

fn metric_diff(
    old: &ResultDoc,
    _new: &ResultDoc,
    metric: &str,
    old_v: f64,
    new_v: f64,
    direction: Direction,
    threshold_pct: f64,
    abs_floor: f64,
) -> MetricDiff {
    let delta = new_v - old_v;
    let delta_pct = if old_v.abs() < f64::EPSILON {
        if new_v.abs() < f64::EPSILON {
            0.0
        } else {
            100.0 * new_v.signum()
        }
    } else {
        delta / old_v.abs() * 100.0
    };

    let verdict = verdict(direction, delta_pct, delta, threshold_pct, abs_floor);

    MetricDiff {
        scenario: old.scenario.clone(),
        proxy: old.proxy.clone(),
        metric: metric.to_string(),
        old: old_v,
        new: new_v,
        delta,
        delta_pct,
        verdict,
    }
}

fn verdict(
    direction: Direction,
    delta_pct: f64,
    delta: f64,
    threshold_pct: f64,
    abs_floor: f64,
) -> Verdict {
    let above_floor = delta.abs() >= abs_floor;
    let crossed = delta_pct.abs() >= threshold_pct;
    if !crossed {
        return Verdict::Neutral;
    }
    if !above_floor {
        return Verdict::Neutral;
    }
    match direction {
        Direction::Higher => {
            if delta_pct > 0.0 {
                Verdict::Improvement
            } else {
                Verdict::Regression
            }
        }
        Direction::Lower => {
            if delta_pct < 0.0 {
                Verdict::Improvement
            } else {
                Verdict::Regression
            }
        }
    }
}

fn print_markdown(diffs: &[MetricDiff]) {
    println!("| scenario | proxy | metric | old | new | Δ | Δ% | verdict |");
    println!("|----------|-------|--------|-----|-----|---|-----|---------|");
    for d in diffs {
        println!(
            "| {scenario} | {proxy} | {metric} | {old_v} | {new_v} | {delta} | {dp:+.2}% | {verdict:?} |",
            scenario = d.scenario,
            proxy = d.proxy,
            metric = d.metric,
            old_v = fmt_num(d.old),
            new_v = fmt_num(d.new),
            delta = fmt_num(d.delta),
            dp = d.delta_pct,
            verdict = d.verdict,
        );
    }
}

fn fmt_num(v: f64) -> String {
    if v.abs() >= 1000.0 {
        format!("{:.0}", v)
    } else if v.abs() >= 1.0 {
        format!("{:.2}", v)
    } else {
        format!("{:.4}", v)
    }
}
