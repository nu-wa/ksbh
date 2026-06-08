//! Tests for the aggregate subcommand.

use ksbh_bench::aggregate::{error_rate_pct, gate, metric_value, run, AggregateArgs};

fn synth_result(scenario: &str, proxy: &str, rps: f64, errors: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "1",
        "scenario": scenario,
        "proxy": proxy,
        "git_sha": "test",
        "machine": {
            "cpus": 4,
            "model": "test",
            "kernel": "1.0",
            "hostname": "h"
        },
        "started_at": "2026-06-06T00:00:00Z",
        "duration_s": 30,
        "concurrency": 100,
        "tls": false,
        "workload": {
            "method": "GET",
            "path": "/",
            "host": "bench.local",
            "body_bytes": 200
        },
        "results": {
            "rps": rps,
            "p50_ms": 0.5,
            "p99_ms": 1.5,
            "max_ms": 5.0,
            "errors": errors,
            "bytes_proxied": 1234567
        },
        "proxy_peak_rss_kb": 12345,
        "proxy_metrics_snapshot": ""
    })
}

#[test]
fn metric_value_reads_results_block() {
    let r = synth_result("speed_small_c1", "ksbh", 1000.0, serde_json::json!({}));
    assert!((metric_value(&r, "rps") - 1000.0).abs() < 1e-9);
    assert!((metric_value(&r, "p99_ms") - 1.5).abs() < 1e-9);
}

#[test]
fn metric_value_sums_errors() {
    let r = synth_result(
        "speed_small_c1",
        "ksbh",
        1000.0,
        serde_json::json!({
            "5xx": 3,
            "4xx": 1,
            "connect_failed": 0,
            "read_timeout": 2
        }),
    );
    assert_eq!(metric_value(&r, "err_5xx"), 3.0);
    assert_eq!(metric_value(&r, "err_4xx"), 1.0);
    assert_eq!(metric_value(&r, "err_connect"), 0.0);
    assert_eq!(metric_value(&r, "err_timeout"), 2.0);
    // 30s * 1000 rps = 30_000 requests; 6 errors => 0.02% rate.
    assert!((metric_value(&r, "error_rate_pct") - 0.02).abs() < 1e-9);
}

#[test]
fn error_rate_pct_computes_correctly() {
    // 30s * 1000 rps = 30_000 requests; 100 errors => 0.333% error rate.
    let r = synth_result(
        "speed_small_c1",
        "ksbh",
        1000.0,
        serde_json::json!({
            "5xx": 50,
            "4xx": 50,
            "connect_failed": 0,
            "read_timeout": 0
        }),
    );
    let pct = error_rate_pct(&r);
    assert!((pct - (100.0 / 30_000.0 * 100.0)).abs() < 1e-6);
}

#[test]
fn gate_fails_on_rps_zero() {
    let r = synth_result("speed_small_c1", "ksbh", 0.0, serde_json::json!({}));
    let results = vec![r];
    assert_eq!(gate(&results), 1);
}

#[test]
fn gate_fails_on_high_error_rate() {
    // 30s * 100 rps = 3000 requests; 500 errors => 16.67% > 10% threshold.
    let r = synth_result(
        "speed_small_c1",
        "ksbh",
        100.0,
        serde_json::json!({
            "5xx": 500,
            "4xx": 0,
            "connect_failed": 0,
            "read_timeout": 0
        }),
    );
    let results = vec![r];
    assert_eq!(gate(&results), 1);
}

#[test]
fn gate_passes_clean_speed_result() {
    let r = synth_result(
        "speed_small_c1",
        "ksbh",
        5000.0,
        serde_json::json!({
            "5xx": 0,
            "4xx": 0,
            "connect_failed": 0,
            "read_timeout": 0
        }),
    );
    let results = vec![r];
    assert_eq!(gate(&results), 0);
}

#[test]
fn gate_ignores_robust_scenarios() {
    // A robust scenario with rps=0 should NOT fail the gate.
    let r = synth_result("robust_slow_loris", "ksbh", 0.0, serde_json::json!({}));
    let results = vec![r];
    assert_eq!(gate(&results), 0);
}

#[test]
fn aggregate_writes_markdown_report() {
    let tmp = std::env::temp_dir().join(format!(
        "ksbh-bench-test-{}",
        std::process::id()
    ));
    let in_dir = tmp.join("results");
    let out_dir = tmp.join("report");
    std::fs::create_dir_all(&in_dir).unwrap();

    let r_ksbh = synth_result(
        "speed_small_c1",
        "ksbh",
        10000.0,
        serde_json::json!({"5xx": 0, "4xx": 0, "connect_failed": 0, "read_timeout": 0}),
    );
    let r_nginx = synth_result(
        "speed_small_c1",
        "nginx",
        9000.0,
        serde_json::json!({"5xx": 0, "4xx": 0, "connect_failed": 0, "read_timeout": 0}),
    );
    std::fs::write(
        in_dir.join("speed_small_c1_ksbh.json"),
        serde_json::to_string_pretty(&r_ksbh).unwrap(),
    )
    .unwrap();
    std::fs::write(
        in_dir.join("speed_small_c1_nginx.json"),
        serde_json::to_string_pretty(&r_nginx).unwrap(),
    )
    .unwrap();

    let args = AggregateArgs {
        in_dir: in_dir.to_string_lossy().to_string(),
        out: out_dir.to_string_lossy().to_string(),
    };
    let code = run(&args).unwrap();
    assert_eq!(code, 0);

    let md = std::fs::read_to_string(out_dir.join("bench.md")).unwrap();
    assert!(md.contains("ksbh bench report"), "header");
    assert!(md.contains("speed_small_c1"), "scenario heading");
    assert!(md.contains("ksbh"), "ksbh column");
    assert!(md.contains("nginx"), "nginx column");

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn aggregate_writes_stitched_json() {
    let tmp = std::env::temp_dir().join(format!(
        "ksbh-bench-test-json-{}",
        std::process::id()
    ));
    let in_dir = tmp.join("results");
    std::fs::create_dir_all(&in_dir).unwrap();
    let r = synth_result(
        "speed_small_c1",
        "ksbh",
        100.0,
        serde_json::json!({"5xx": 0, "4xx": 0, "connect_failed": 0, "read_timeout": 0}),
    );
    std::fs::write(
        in_dir.join("speed_small_c1_ksbh.json"),
        serde_json::to_string_pretty(&r).unwrap(),
    )
    .unwrap();

    let out_path = tmp.join("history").join("aaaa.json");
    let args = AggregateArgs {
        in_dir: in_dir.to_string_lossy().to_string(),
        out: out_path.to_string_lossy().to_string(),
    };
    let code = run(&args).unwrap();
    assert_eq!(code, 0);

    let raw = std::fs::read_to_string(&out_path).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(doc["schema_version"], "1");
    assert_eq!(doc["result_count"], 1);
    assert_eq!(doc["results"][0]["scenario"], "speed_small_c1");

    let _ = std::fs::remove_dir_all(&tmp);
}
