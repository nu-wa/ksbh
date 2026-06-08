//! Focused unit tests for the router, driven by [`crate::routing::test_utils`].
//!
//! These cover the routing seams (`RouterReader::find_route`, the merge of
//! global + ingress-specific module chains, and the `HostPaths::find` arms)
//! without spinning up an e2e harness.

use std::sync::Arc;

use crate::routing::test_utils::{HostPathsBuilder, RouterHarness, test_module_spec};
use crate::routing::RoutingDestination;

#[test]
fn exact_path_match_returns_some_request_match() {
    let mut harness = RouterHarness::new();

    harness.add_host(
        "example.local",
        HostPathsBuilder::new()
            .exact("/foo", RoutingDestination::Static)
            .build(),
    );

    let result = harness.find_route("example.local", "/foo");

    let m = result.expect("expected Some(RequestMatch) for exact match GET /foo");
    assert_eq!(m.destination, RoutingDestination::Static);
}

#[test]
fn prefix_path_match_returns_some_request_match() {
    let mut harness = RouterHarness::new();

    harness.add_host(
        "example.local",
        HostPathsBuilder::new()
            .prefix("/api", RoutingDestination::Static)
            .build(),
    );

    let result = harness.find_route("example.local", "/api/users");

    let m = result.expect("expected Some(RequestMatch) for prefix match GET /api/users");
    assert_eq!(m.destination, RoutingDestination::Static);
}

#[test]
fn unknown_host_returns_none() {
    let harness = RouterHarness::new();

    let result = harness.find_route("nonexistent.local", "/");

    assert!(
        result.is_none(),
        "expected None for unconfigured host, got: {:?}",
        result.map(|m| m.destination)
    );
}

#[test]
fn unknown_path_returns_none() {
    let mut harness = RouterHarness::new();

    harness.add_host(
        "example.local",
        HostPathsBuilder::new()
            .exact("/foo", RoutingDestination::Static)
            .build(),
    );

    let result = harness.find_route("example.local", "/bar");

    assert!(
        result.is_none(),
        "expected None for unconfigured path on configured host, got: {:?}",
        result.map(|m| m.destination)
    );
}

#[test]
fn global_module_chain_is_included_in_request_match() {
    let mut harness = RouterHarness::new();

    harness.add_global_module("global-1", test_module_spec("global-1", 10, true));

    harness.add_host(
        "example.local",
        HostPathsBuilder::new()
            .exact("/foo", RoutingDestination::Static)
            .build(),
    );

    let m = harness
        .find_route("example.local", "/foo")
        .expect("expected Some(RequestMatch)");

    let names: Vec<String> = m.modules.iter().map(|module| module.name.to_string()).collect();
    assert_eq!(names, vec!["global-1".to_string()]);
}

#[test]
fn ingress_specific_module_chain_overrides_global() {
    let mut harness = RouterHarness::new();

    harness
        .add_global_module("global-low", test_module_spec("global-low", 10, true))
        .add_global_module("global-high", test_module_spec("global-high", 100, true))
        .add_module("ingress-high", test_module_spec("ingress-high", 1000, false))
        .add_module("ingress-low", test_module_spec("ingress-low", 1, false));

    harness.add_host_with_modules(
        "example.local",
        HostPathsBuilder::new()
            .exact("/foo", RoutingDestination::Static)
            .build(),
        vec![Arc::from("ingress-low"), Arc::from("ingress-high")],
    );

    let m = harness
        .find_route("example.local", "/foo")
        .expect("expected Some(RequestMatch)");

    let names: Vec<String> = m.modules.iter().map(|module| module.name.to_string()).collect();

    // Global modules come first (sorted by weight desc, then name asc),
    // followed by ingress-specific modules (same sort).
    assert_eq!(
        names,
        vec![
            "global-high".to_string(),
            "global-low".to_string(),
            "ingress-high".to_string(),
            "ingress-low".to_string(),
        ]
    );
}

#[test]
fn implementation_specific_path_does_not_match_unrelated_prefix() {
    let mut harness = RouterHarness::new();

    harness.add_host(
        "example.local",
        HostPathsBuilder::new()
            .implementation_specific("/ap", RoutingDestination::Static)
            .build(),
    );

    let result = harness.find_route("example.local", "/api/v1");

    assert!(
        result.is_none(),
        "expected None for path '/api/v1' with implementation_specific '/ap', got: {:?}",
        result.map(|m| m.destination)
    );
}
