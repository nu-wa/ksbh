//! Aggregate `bench/results/*.json` into a Markdown report or a stitched
//! history JSON.
//!
//! The output mode is selected by `--out`:
//!
//! * `--out` is a directory  -> emit `<dir>/bench.md` (the per-scenario report).
//! * `--out` ends with `.json` -> emit a single stitched JSON object suitable
//!   for `bench/history/<sha>.json`.
//!
//! Exits non-zero if any speed scenario has `rps == 0` or error rate > 10% —
//! the same gate the original Python aggregator enforced, so the bench stays
//! drop-in compatible with CI.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde::Serialize;

/// Scenarios whose results count toward the smoke gate. The four `robust_*`
/// scenarios are adversarial and are excluded.
const SPEED_SCENARIOS: &[&str] = &[
    "speed_small_c1",
    "speed_small_c100",
    "speed_large_c50",
    "speed_tls_h2",
    "speed_const_50krps",
];

/// Per-row error rate over 10% on a speed scenario fails the gate.
const ERROR_RATE_PCT_THRESHOLD: f64 = 10.0;

/// Columns rendered in the Markdown report, in display order.
const METRIC_ROWS: &[(&str, &str)] = &[
    ("rps",            "RPS"),
    ("p50_ms",         "p50 (ms)"),
    ("p99_ms",         "p99 (ms)"),
    ("max_ms",         "max (ms)"),
    ("peak_rss_kb",    "peak RSS (KiB)"),
    ("err_5xx",        "errors · 5xx"),
    ("err_4xx",        "errors · 4xx"),
    ("err_connect",    "errors · connect_failed"),
    ("err_timeout",    "errors · read_timeout"),
    ("error_rate_pct", "error rate"),
];

#[derive(Debug, Clone)]
pub struct AggregateArgs {
    pub in_dir: String,
    pub out: String,
}

#[derive(Debug, Serialize)]
struct StitchedDoc {
    schema_version: String,
    git_sha: String,
    generated_at: String,
    machine: Option<serde_json::Value>,
    result_count: usize,
    results: Vec<serde_json::Value>,
}

pub fn run(args: &AggregateArgs) -> Result<i32> {
    let in_dir = Path::new(&args.in_dir);
    if !in_dir.is_dir() {
        anyhow::bail!("--in directory does not exist: {}", in_dir.display());
    }

    let results = load_results(in_dir)?;
    if results.is_empty() {
        eprintln!("warning: no result files in {}", in_dir.display());
    }

    let out = PathBuf::from(&args.out);
    let out_is_json = args.out.ends_with(".json");
    if out_is_json {
        let path = render_stitched_json(&results, &out)?;
        println!("wrote {}", path.display());
    } else {
        let path = render_markdown(&results, &out)?;
        println!("wrote {}", path.display());
    }

    Ok(gate(&results))
}

fn load_results(in_dir: &Path) -> Result<Vec<serde_json::Value>> {
    let mut out = Vec::new();
    let mut entries: Vec<PathBuf> = fs::read_dir(in_dir)
        .with_context(|| format!("reading dir {}", in_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    entries.sort();
    for path in entries {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let v: serde_json::Value = serde_json::from_str(&raw)
            .with_context(|| format!("parsing {}", path.display()))?;
        out.push(v);
    }
    out.sort_by(|a, b| {
        let ka = scenario_key(a);
        let kb = scenario_key(b);
        ka.cmp(&kb)
    });
    Ok(out)
}

fn scenario_key(v: &serde_json::Value) -> (String, String) {
    (
        v.get("scenario").and_then(|s| s.as_str()).unwrap_or("").to_string(),
        v.get("proxy").and_then(|s| s.as_str()).unwrap_or("").to_string(),
    )
}

pub fn metric_value(result: &serde_json::Value, key: &str) -> f64 {
    if key == "peak_rss_kb" {
        return result
            .get("proxy_peak_rss_kb")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
    }
    if key == "error_rate_pct" {
        return error_rate_pct(result);
    }
    if let Some(field) = match key {
        "err_5xx" => Some("5xx"),
        "err_4xx" => Some("4xx"),
        "err_connect" => Some("connect_failed"),
        "err_timeout" => Some("read_timeout"),
        _ => None,
    } {
        return result
            .pointer(&format!("/results/errors/{}", field))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
    }
    result
        .pointer(&format!("/results/{}", key))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
}

pub fn error_rate_pct(result: &serde_json::Value) -> f64 {
    let errs = result
        .pointer("/results/errors")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let total_err: f64 = match errs.as_object() {
        Some(o) => ["5xx", "4xx", "connect_failed", "read_timeout"]
            .iter()
            .map(|k| o.get(*k).and_then(|v| v.as_f64()).unwrap_or(0.0))
            .sum(),
        None => 0.0,
    };
    let rps = result
        .pointer("/results/rps")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let duration = result
        .get("duration_s")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let total_req = rps * duration;
    if total_req <= 0.0 {
        return 0.0;
    }
    total_err / total_req * 100.0
}

fn git_sha() -> String {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success());
    match out {
        Some(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        None => "unknown".to_string(),
    }
}

fn render_markdown(results: &[serde_json::Value], out_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(out_dir)
        .with_context(|| format!("creating report dir {}", out_dir.display()))?;
    let md_path = out_dir.join("bench.md");

    let mut by_scenario: std::collections::BTreeMap<String, std::collections::BTreeMap<String, &serde_json::Value>> =
        std::collections::BTreeMap::new();
    for r in results {
        let s = r.get("scenario").and_then(|s| s.as_str()).unwrap_or("").to_string();
        let p = r.get("proxy").and_then(|s| s.as_str()).unwrap_or("").to_string();
        by_scenario.entry(s).or_default().insert(p, r);
    }

    let mut lines: Vec<String> = Vec::new();
    lines.push("# ksbh bench report".to_string());
    lines.push(String::new());
    lines.push(format!("- generated: {}", now_iso()));
    lines.push(format!("- git SHA:   {}", git_sha()));
    lines.push(format!(
        "- result files: {} ({} scenarios)",
        results.len(),
        by_scenario.len()
    ));
    if let Some(first) = results.first() {
        let m = first.get("machine").cloned().unwrap_or(serde_json::Value::Null);
        let m = m.as_object();
        let cpus = m
            .and_then(|o| o.get("cpus"))
            .and_then(|v| v.as_i64())
            .map(|n| n.to_string())
            .unwrap_or_else(|| "?".to_string());
        let model = m
            .and_then(|o| o.get("model"))
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let kernel = m
            .and_then(|o| o.get("kernel"))
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let hostname = m
            .and_then(|o| o.get("hostname"))
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        lines.push(format!(
            "- machine:   {} CPUs, {} ({}) @ {}",
            cpus, model, kernel, hostname
        ));
    }
    lines.push(String::new());

    for (scenario, proxies) in &by_scenario {
        let ksbh = proxies.get("ksbh").copied();
        let nginx = proxies.get("nginx").copied();
        lines.push(format!("## `{}`", scenario));
        lines.push(String::new());
        lines.push("| metric | ksbh | nginx | delta | delta % |".to_string());
        lines.push("|---|---:|---:|---:|---:|".to_string());
        for (key, label) in METRIC_ROWS {
            let k = ksbh.map(|r| metric_value(r, key));
            let n = nginx.map(|r| metric_value(r, key));
            if k.is_none() && n.is_none() {
                continue;
            }
            let k_str = k.map(|v| fmt_metric(key, v)).unwrap_or_else(|| "—".to_string());
            let n_str = n.map(|v| fmt_metric(key, v)).unwrap_or_else(|| "—".to_string());
            let (d_str, p_str) = match (k, n) {
                (Some(k), Some(n)) if k != 0.0 => {
                    let d = n - k;
                    let pct = d / k * 100.0;
                    (fmt_signed(d), format!("{:.2}%", pct))
                }
                (Some(_), Some(_)) => ("—".to_string(), "—".to_string()),
                _ => ("—".to_string(), "—".to_string()),
            };
            lines.push(format!(
                "| {} | {} | {} | {} | {} |",
                label, k_str, n_str, d_str, p_str
            ));
        }
        lines.push(String::new());
    }

    lines.push("## Failures".to_string());
    lines.push(String::new());
    let mut failures: Vec<String> = Vec::new();
    for (scenario, proxies) in &by_scenario {
        if !SPEED_SCENARIOS.contains(&scenario.as_str()) {
            continue;
        }
        for (proxy, r) in proxies {
            let rps = metric_value(r, "rps");
            let err_pct = error_rate_pct(r);
            let mut reason: Vec<String> = Vec::new();
            if rps <= 0.0 {
                reason.push("rps=0 (silent failure)".to_string());
            }
            if err_pct > ERROR_RATE_PCT_THRESHOLD {
                reason.push(format!(
                    "error rate {:.2}% > {:.0}%",
                    err_pct, ERROR_RATE_PCT_THRESHOLD
                ));
            }
            if !reason.is_empty() {
                failures.push(format!(
                    "- `{}` / `{}`: {}",
                    scenario,
                    proxy,
                    reason.join("; ")
                ));
            }
        }
    }
    if failures.is_empty() {
        lines.push("_none_".to_string());
    } else {
        lines.extend(failures);
    }
    lines.push(String::new());

    let body = lines.join("\n");
    fs::write(&md_path, body).with_context(|| format!("writing {}", md_path.display()))?;
    Ok(md_path)
}

fn render_stitched_json(results: &[serde_json::Value], out_path: &Path) -> Result<PathBuf> {
    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating dir {}", parent.display()))?;
        }
    }
    let machine = results
        .first()
        .and_then(|r| r.get("machine").cloned())
        .unwrap_or(serde_json::Value::Null);
    let doc = StitchedDoc {
        schema_version: "1".to_string(),
        git_sha: git_sha(),
        generated_at: now_iso(),
        machine: if machine.is_null() { None } else { Some(machine) },
        result_count: results.len(),
        results: results.to_vec(),
    };
    let json = serde_json::to_string_pretty(&doc)?;
    fs::write(out_path, format!("{}\n", json))
        .with_context(|| format!("writing {}", out_path.display()))?;
    Ok(out_path.to_path_buf())
}

pub fn gate(results: &[serde_json::Value]) -> i32 {
    for r in results {
        let scenario = r
            .get("scenario")
            .and_then(|s| s.as_str())
            .unwrap_or("");
        if !SPEED_SCENARIOS.contains(&scenario) {
            continue;
        }
        let rps = metric_value(r, "rps");
        if rps <= 0.0 {
            eprintln!(
                "gate fail: {}/{} rps=0",
                scenario,
                r.get("proxy").and_then(|s| s.as_str()).unwrap_or("?")
            );
            return 1;
        }
        let pct = error_rate_pct(r);
        if pct > ERROR_RATE_PCT_THRESHOLD {
            eprintln!(
                "gate fail: {}/{} error rate {:.2}% > {:.0}%",
                scenario,
                r.get("proxy").and_then(|s| s.as_str()).unwrap_or("?"),
                pct,
                ERROR_RATE_PCT_THRESHOLD
            );
            return 1;
        }
    }
    0
}

fn now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // ISO 8601 in UTC, second resolution. We don't need sub-second here; the
    // upstream `started_at` field is sub-second but generated_at is just
    // "when this report ran" and is fine at second resolution.
    let days = (secs / 86_400) as i64;
    let mut year = 1970i64;
    let mut remaining_days = days;
    loop {
        let leap = is_leap(year);
        let yd = if leap { 366 } else { 365 };
        if remaining_days < yd {
            break;
        }
        remaining_days -= yd;
        year += 1;
    }
    let leap = is_leap(year);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 0usize;
    for (i, &d) in month_days.iter().enumerate() {
        if remaining_days < d {
            month = i;
            break;
        }
        remaining_days -= d;
    }
    let day = remaining_days + 1;
    let secs_today = (secs % 86_400) as u32;
    let hh = secs_today / 3600;
    let mm = (secs_today % 3600) / 60;
    let ss = secs_today % 60;
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year,
        month + 1,
        day,
        hh,
        mm,
        ss
    )
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn fmt_num(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1.0e15 {
        return format!("{}", v as i64);
    }
    let s = format!("{:.3}", v);
    s
}

fn fmt_metric(key: &str, v: f64) -> String {
    if key == "error_rate_pct" {
        return format!("{:.5}%", v);
    }
    fmt_num(v)
}

fn fmt_signed(v: f64) -> String {
    if v == 0.0 {
        return "0".to_string();
    }
    let s = format!("{:+.3}", v);
    // Strip a trailing .000 or sign-only artifacts for readability.
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" {
        "0".to_string()
    } else {
        s
    }
}
