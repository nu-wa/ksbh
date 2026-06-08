//! Tests for the compare subcommand's verdict logic.

use ksbh_bench::analyze::{Errors, Results, ResultDoc, Workload, MachineInfo};
use ksbh_bench::compare::{compare_pair, ScenarioClass, Thresholds, Verdict};

fn make_doc(scenario: &str, rps: f64, p50_ms: f64, p99_ms: f64, rss_kb: u64) -> ResultDoc {
    ResultDoc {
        schema_version: "1".into(),
        scenario: scenario.into(),
        proxy: "ksbh".into(),
        git_sha: "deadbeef".into(),
        machine: MachineInfo {
            cpus: 1,
            model: "test".into(),
            kernel: "test".into(),
            hostname: "test".into(),
        },
        started_at: "2026-06-06T00:00:00Z".into(),
        duration_s: 30,
        concurrency: 100,
        tls: false,
        workload: Workload {
            method: "GET".into(),
            path: "/".into(),
            host: "bench.local".into(),
            body_bytes: 200,
        },
        results: Results {
            rps,
            p50_ms,
            p99_ms,
            max_ms: 1.0,
            errors: Errors::default(),
            bytes_proxied: 1000,
        },
        proxy_peak_rss_kb: rss_kb,
        proxy_metrics_snapshot: "{}".into(),
    }
}

#[test]
fn class_inference() {
    assert_eq!(ScenarioClass::for_scenario("speed_small_c1"), ScenarioClass::Speed);
    assert_eq!(ScenarioClass::for_scenario("speed_tls_h2_c50"), ScenarioClass::SpeedTlsH2);
    assert_eq!(ScenarioClass::for_scenario("robust_slow_loris"), ScenarioClass::Robust);
    assert_eq!(ScenarioClass::for_scenario("speed_large_c50"), ScenarioClass::Speed);
}

#[test]
fn rps_gain_on_robust_is_neutral() {
    let old = make_doc("robust_slow_loris", 1000.0, 10.0, 50.0, 100_000);
    let new = make_doc("robust_slow_loris", 1170.0, 10.0, 50.0, 100_000);
    let diffs = compare_pair(&old, &new);
    let rps = diffs.iter().find(|d| d.metric == "rps").unwrap();
    // 17% gain; Robust threshold is 25% -> NEUTRAL.
    assert!((rps.delta_pct - 17.0).abs() < 0.01);
    assert_eq!(rps.verdict, Verdict::Neutral);
}

#[test]
fn rps_loss_on_speed_is_regression() {
    let old = make_doc("speed_small_c1", 100_000.0, 0.5, 1.0, 50_000);
    let new = make_doc("speed_small_c1", 95_000.0, 0.5, 1.0, 50_000);
    let diffs = compare_pair(&old, &new);
    let rps = diffs.iter().find(|d| d.metric == "rps").unwrap();
    // 5% loss; Speed threshold is 5% -> REGRESSION.
    assert!((rps.delta_pct - (-5.0)).abs() < 0.01);
    assert_eq!(rps.verdict, Verdict::Regression);
}

#[test]
fn rps_gain_on_speed_is_improvement() {
    let old = make_doc("speed_small_c1", 100_000.0, 0.5, 1.0, 50_000);
    let new = make_doc("speed_small_c1", 110_000.0, 0.5, 1.0, 50_000);
    let diffs = compare_pair(&old, &new);
    let rps = diffs.iter().find(|d| d.metric == "rps").unwrap();
    // 10% gain; Speed threshold is 5% -> IMPROVEMENT.
    assert_eq!(rps.verdict, Verdict::Improvement);
}

#[test]
fn threshold_for_classes() {
    let speed = Thresholds::for_class(ScenarioClass::Speed);
    let tls = Thresholds::for_class(ScenarioClass::SpeedTlsH2);
    let robust = Thresholds::for_class(ScenarioClass::Robust);
    assert_eq!(speed.rps_pct, 5.0);
    assert_eq!(tls.rps_pct, 15.0);
    assert_eq!(robust.rps_pct, 25.0);
    assert!(robust.p99_abs_floor_ms > 100.0);
}
