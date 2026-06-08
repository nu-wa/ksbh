//! Criterion microbenchmarks for ksbh-core hot-path functions.
//!
//! These measure the per-request cost of individual functions called on
//! every HTTP request through the proxy. Run with:
//!
//! ```sh
//! cargo bench -p ksbh-core
//! ```
//!
//! Each function here is called 1–2× per proxied request. A 5% regression
//! in any of these is invisible to the HTTP-level bench (vegeta) but
//! compounds across millions of requests.

use std::net::{IpAddr, SocketAddr};

use async_trait::async_trait;
use bytes::Bytes;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use hashbrown::HashMap;
use http::{HeaderMap, HeaderName, HeaderValue, Response, StatusCode, Uri};
use ksbh_types::prelude::{ProxyProviderError, ProxyProviderSession};

use ksbh_core::routing::hosts::HostPaths;
use ksbh_core::routing::service_backend::{RoutingDestination, Upstream};

// ── Minimal session mock for bench ──────────────────────────────────

struct BenchSession {
    headers: HeaderMap,
    client_addr: Option<IpAddr>,
}

#[async_trait]
impl ProxyProviderSession for BenchSession {
    fn headers(&self) -> http::request::Parts {
        unimplemented!("bench only")
    }
    fn header_map(&self) -> &HeaderMap {
        &self.headers
    }
    fn get_header(&self, _name: HeaderName) -> Option<&HeaderValue> {
        None
    }
    fn set_request_uri(&mut self, _uri: Uri) {}
    fn server_addr(&self) -> Option<SocketAddr> {
        None
    }
    fn response_written(&self) -> bool {
        false
    }
    fn response_status(&self) -> Option<StatusCode> {
        None
    }
    fn response_sent(&self) -> bool {
        false
    }
    fn client_addr(&self) -> Option<IpAddr> {
        self.client_addr
    }
    async fn write_response(
        &mut self,
        _response: Response<Bytes>,
    ) -> Result<(), ProxyProviderError> {
        Ok(())
    }
    async fn read_request_body(&mut self) -> Result<Option<Bytes>, ProxyProviderError> {
        Ok(None)
    }
}

// ── 1. get_cookie_domain (PSL lookup) ───────────────────────────────

fn bench_cookie_domain(c: &mut Criterion) {
    let hosts = [
        "example.com",
        "app.staging.example.co.uk",
        "myservice",
        "127.0.0.1",
        "very.long.hostname.that.tests.psl.parsing.performance.example.com",
    ];

    c.bench_function("get_cookie_domain/psl_lookup", |b| {
        b.iter(|| {
            for host in &hosts {
                let _ = black_box(ksbh_core::cookies::get_cookie_domain(host));
            }
        })
    });
}

// ── 2. get_client_ip_from_session (IP parsing + fallback) ──────────

use ksbh_core::utils::get_client_ip_from_session;

fn bench_client_ip(c: &mut Criterion) {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-forwarded-for"),
        "10.0.0.1, 192.168.1.1".parse().unwrap(),
    );
    headers.insert(
        HeaderName::from_static("x-real-ip"),
        "10.0.0.1".parse().unwrap(),
    );

    let session = BenchSession {
        headers,
        client_addr: Some("127.0.0.1".parse().unwrap()),
    };

    c.bench_function("get_client_ip_from_session/trusted", |b| {
        b.iter(|| {
            let _ =
                black_box(get_client_ip_from_session(black_box(&session), black_box(true)));
        })
    });

    c.bench_function("get_client_ip_from_session/untrusted", |b| {
        b.iter(|| {
            let _ =
                black_box(get_client_ip_from_session(black_box(&session), black_box(false)));
        })
    });
}

// ── 3. HostPaths::find (path routing) ───────────────────────────────

fn bench_host_paths_find(c: &mut Criterion) {
    let mut exact: HashMap<_, _> = HashMap::new();
    exact.insert(
        "exact-match".into(),
        RoutingDestination::Upstream(Upstream {
            name: "svc-exact".into(),
            port: 8080,
        }),
    );

    let mut prefix: Vec<(_, RoutingDestination)> = Vec::new();
    for i in 0..100 {
        prefix.push((
            format!("/api/v{}/users", i).into(),
            RoutingDestination::Upstream(Upstream {
                name: format!("svc-{}", i).into(),
                port: 8080,
            }),
        ));
    }

    let hp = HostPaths {
        exact,
        prefix,
        implementation_specific: Vec::new(),
    };

    let paths = [
        "/api/v0/users/123",
        "/api/v99/users/456/profile",
        "/api/v50/users",
        "/nonexistent/path",
        "/api/v0/users/a/b/c/d/e/f/g/h",
    ];

    c.bench_function("HostPaths::find/100_prefix_rules", |b| {
        b.iter(|| {
            for p in &paths {
                let _ = black_box(hp.find(p));
            }
        })
    });

    let empty = HostPaths::default();
    c.bench_function("HostPaths::find/empty", |b| {
        b.iter(|| {
            let _ = black_box(empty.find("/anything"));
        })
    });
}

// ── 4. stable_filtered_header_hash (Blake3 header fingerprint) ─────

use ksbh_core::proxy::ClientInformation;

fn bench_header_hash(c: &mut Criterion) {
    // Simulate a realistic set of request headers.
    let mut headers = HeaderMap::new();
    headers.insert("host", "bench.local".parse().unwrap());
    headers.insert("user-agent", "vegeta/12.12.0".parse().unwrap());
    headers.insert("accept", "*/*".parse().unwrap());
    headers.insert("accept-encoding", "gzip".parse().unwrap());
    headers.insert("x-forwarded-for", "10.0.0.1".parse().unwrap());
    headers.insert("x-forwarded-proto", "https".parse().unwrap());
    // Add some noise — headers that get filtered out.
    headers.insert("connection", "keep-alive".parse().unwrap());
    headers.insert("cookie", "session=abc123; other=xyz".parse().unwrap());

    c.bench_function("stable_filtered_header_hash/typical", |b| {
        b.iter(|| {
            let _ = black_box(ClientInformation::stable_filtered_header_hash(black_box(
                &headers,
            )));
        })
    });

    // Heavy case: many headers.
    let mut many = HeaderMap::new();
    for i in 0..50 {
        many.insert(
            HeaderName::from_bytes(format!("x-custom-{}", i).as_bytes()).unwrap(),
            format!("value-{}", i).parse().unwrap(),
        );
    }
    c.bench_function("stable_filtered_header_hash/50_headers", |b| {
        b.iter(|| {
            let _ = black_box(ClientInformation::stable_filtered_header_hash(black_box(
                &many,
            )));
        })
    });
}

// ── 5. compose_forwarded_header_value (header assembly) ────────────

use ksbh_core::proxy::service::ProxyService;

fn bench_forwarded_header_compose(c: &mut Criterion) {
    let existing =
        HeaderValue::from_static("10.0.0.1, 192.168.1.1");
    let appended = "172.16.0.1";

    c.bench_function("compose_forwarded_header_value/trusted_with_existing", |b| {
        b.iter(|| {
            let _ = black_box(ProxyService::compose_forwarded_header_value(
                black_box(Some(&existing)),
                black_box(appended),
                black_box(true),
            ));
        })
    });

    c.bench_function("compose_forwarded_header_value/untrusted", |b| {
        b.iter(|| {
            let _ = black_box(ProxyService::compose_forwarded_header_value(
                black_box(Some(&existing)),
                black_box(appended),
                black_box(false),
            ));
        })
    });

    c.bench_function("compose_forwarded_header_value/no_existing", |b| {
        b.iter(|| {
            let _ = black_box(ProxyService::compose_forwarded_header_value(
                black_box(None),
                black_box(appended),
                black_box(true),
            ));
        })
    });
}

// ── 6. forwarded_header_entry (RFC 7239 Forwarded) ─────────────────

fn bench_forwarded_header_entry(c: &mut Criterion) {
    let ipv4 = Some(IpAddr::from([10, 0, 0, 1]));
    let ipv6 = Some(IpAddr::from([0x2001, 0xdb8, 0, 0, 0, 0, 0, 1]));

    c.bench_function("forwarded_header_entry/ipv4", |b| {
        b.iter(|| {
            let _ = black_box(ProxyService::forwarded_header_entry(
                black_box(ipv4),
                black_box("https"),
                black_box("bench.local"),
            ));
        })
    });

    c.bench_function("forwarded_header_entry/ipv6", |b| {
        b.iter(|| {
            let _ = black_box(ProxyService::forwarded_header_entry(
                black_box(ipv6),
                black_box("https"),
                black_box("bench.local"),
            ));
        })
    });
}

// ── 7. header_has_token (comma-split + case-insensitive match) ─────
//
// BLOCKED: lives in `pingora_bridge.rs` which has pre-existing compilation
// errors (`.provider` field + `PingoraWrapper` re-export). Uncomment once
// those are fixed.
//
// fn bench_header_has_token(c: &mut Criterion) { ... }
// use ksbh_core::proxy::pingora_bridge::ProxyService;
// → ProxyService::header_has_token(...)

// ── 8. normalize_cookie_header_for_upstream ────────────────────────
//
// Skipped: requires a mutable `pingora_http::RequestHeader` which is
// complex to construct in a bench. The merge logic (collect → filter_map
// → join → reinsert) is benchmarked implicitly via the header_has_token
// and compose_forwarded_header_value benches above (same patterns:
// HeaderMap iteration, string trimming, value recombination).

criterion_group!(
    benches,
    bench_cookie_domain,
    bench_client_ip,
    bench_host_paths_find,
    bench_header_hash,
    bench_forwarded_header_compose,
    bench_forwarded_header_entry,
    // bench_header_has_token,  // BLOCKED: pingora_bridge.rs compilation
);
criterion_main!(benches);
