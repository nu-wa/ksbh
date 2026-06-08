//! Golden-file tests for the render subcommand.

use std::path::PathBuf;

use ksbh_bench::render::{render_scenario, RenderCtx};
use ksbh_bench::scenario::Scenario;

fn ctx() -> RenderCtx {
    RenderCtx {
        proxy_host: "bench.local".into(),
        upstream_port: 18080,
        proxy_http_port: 18081,
        proxy_https_port: 18443,
        proxy_internal_port: 18082,
        proxy_metrics_port: 19090,
        nginx_http_port: 18083,
        cert_path: "/etc/bench/cert.pem".into(),
        key_path: "/etc/bench/key.pem".into(),
        host_cert_path: "/tmp/bench/cert.pem".into(),
        host_key_path: "/tmp/bench/key.pem".into(),
    }
}

fn load(name: &str) -> Scenario {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "bench",
        "scenarios",
        name,
    ]
    .iter()
    .collect();
    let raw = std::fs::read_to_string(&path).expect("read scenario");
    toml::from_str(&raw).expect("parse scenario")
}

#[test]
fn small_c1_renders() {
    let scn = load("speed_small_c1.toml");
    let out = render_scenario(&scn, &ctx()).unwrap();

    // vegeta.targets uses substituted port.
    assert!(
        out.vegeta_targets.contains("bench.local:18081/"),
        "vegeta.targets = {}",
        out.vegeta_targets
    );

    // ksbh.yaml points to host cert paths and the upstream port.
    assert!(out.ksbh_yaml.contains("name: bench"));
    assert!(out.ksbh_yaml.contains("host: bench.local"));
    assert!(out.ksbh_yaml.contains("cert_file: /tmp/bench/cert.pem"));
    assert!(out.ksbh_yaml.contains("key_file: /tmp/bench/key.pem"));
    assert!(out.ksbh_yaml.contains("port: 18080"));

    // nginx.conf has the upstream port, healthz endpoint, and the default
    // proxy_buffer directives (8k / "4 8k").
    assert!(out.nginx_conf.contains("listen 18083;"));
    assert!(out.nginx_conf.contains("proxy_pass http://127.0.0.1:18080;"));
    assert!(out.nginx_conf.contains("proxy_buffer_size 8k;"));
    assert!(out.nginx_conf.contains("proxy_buffers 4 8k;"));
    assert!(out.nginx_conf.contains("location = /__bench_healthz"));
    // No TLS block for this scenario.
    assert!(!out.nginx_conf.contains("ssl http2"));
}

#[test]
fn large_c50_uses_overrides() {
    let scn = load("speed_large_c50.toml");
    let out = render_scenario(&scn, &ctx()).unwrap();

    // 64 KiB body, so buffer overrides from the TOML take effect.
    assert!(
        out.vegeta_targets.contains("?bytes=65536"),
        "got: {}",
        out.vegeta_targets
    );
    assert!(out.nginx_conf.contains("proxy_buffer_size 16k;"));
    assert!(out.nginx_conf.contains("proxy_buffers 8 16k;"));
}

#[test]
fn tls_h2_renders_ssl_block() {
    let scn = load("speed_tls_h2.toml");
    let out = render_scenario(&scn, &ctx()).unwrap();

    // Second server block exists with ssl http2 and the https port.
    assert!(out.nginx_conf.contains("listen 18443 ssl http2;"));
    assert!(out.nginx_conf.contains("ssl_certificate /etc/bench/cert.pem;"));
    assert!(out.nginx_conf.contains("ssl_certificate_key /etc/bench/key.pem;"));

    // vegeta.targets uses https + the https port.
    assert!(
        out.vegeta_targets.contains("https://bench.local:18443/"),
        "got: {}",
        out.vegeta_targets
    );
}
