//! Tests for the analyze subcommand's vegeta parsing.

use ksbh_bench::analyze::analyze_vegeta;

fn synth_vegeta() -> serde_json::Value {
    serde_json::json!({
        "rate": 12345.6,
        "latencies": {
            "50th": 500_000u64,
            "99th": 1_200_000u64,
            "max": 5_000_000u64
        },
        "bytes_in": { "total": 1_234_567u64 },
        "bytes_out": { "total": 1_234_567u64 },
        "status_codes": { "200": 12345u64, "502": 5u64 },
        "errors": [
            "Get \"http://bench.local/\": lookup bench.local: no such host",
            "Get \"http://bench.local/\": read tcp: context deadline exceeded"
        ]
    })
}

#[test]
fn parses_rps_and_latencies() {
    let v = synth_vegeta();
    let r = analyze_vegeta(&v).unwrap();
    assert!((r.rps - 12345.6).abs() < 1e-6);
    // 500_000 ns = 0.5 ms
    assert!((r.p50_ms - 0.5).abs() < 1e-9, "p50_ms={}", r.p50_ms);
    // 1_200_000 ns = 1.2 ms
    assert!((r.p99_ms - 1.2).abs() < 1e-9, "p99_ms={}", r.p99_ms);
    // 5_000_000 ns = 5.0 ms
    assert!((r.max_ms - 5.0).abs() < 1e-9, "max_ms={}", r.max_ms);
}

#[test]
fn sums_bytes() {
    let v = synth_vegeta();
    let r = analyze_vegeta(&v).unwrap();
    assert_eq!(r.bytes_proxied, 1_234_567 * 2);
}

#[test]
fn counts_status_codes_by_prefix() {
    let v = synth_vegeta();
    let r = analyze_vegeta(&v).unwrap();
    assert_eq!(r.errors.s5xx, 5);
    assert_eq!(r.errors.s4xx, 0);
}

#[test]
fn classifies_error_messages() {
    let v = synth_vegeta();
    let r = analyze_vegeta(&v).unwrap();
    assert_eq!(r.errors.connect_failed, 1, "no such host");
    assert_eq!(r.errors.read_timeout, 1, "context deadline exceeded");
}
