//! Tiny `{{ var }}` substitution.
//!
//! Replaces `{{ name }}` (whitespace around the name is allowed) with
//! the value of `name` from the [`RenderCtx`]. Unknown names are left
//! as-is so a typo doesn't silently drop content.

use crate::render::RenderCtx;

/// Render `{{ name }}` placeholders in `template` against `ctx`.
///
/// Unknown placeholders are left untouched. This is a deliberate trade-off:
/// the inputs (scenario TOML, nginx template strings) are small and the
/// risk of silently swallowing a typo is worse than a render that fails
/// later when a missing port fails nginx config validation.
pub fn render(template: &str, ctx: &RenderCtx) -> String {
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            // Find closing `}}`
            if let Some(close_rel) = find_close(&bytes[i + 2..]) {
                let inner = &template[i + 2..i + 2 + close_rel];
                let name = inner.trim();
                if let Some(value) = ctx.lookup(name) {
                    out.push_str(&value);
                } else {
                    // Leave the original token in place.
                    out.push_str(&template[i..i + 2 + close_rel + 2]);
                }
                i += 2 + close_rel + 2;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Returns the offset of the closing `}}` relative to `bytes`, or `None` if
/// no closing delimiter is found before end of input.
fn find_close(bytes: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'}' && bytes[i + 1] == b'}' {
            return Some(i);
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> RenderCtx {
        RenderCtx {
            proxy_host: "bench.local".into(),
            upstream_port: 8080,
            proxy_http_port: 80,
            proxy_https_port: 443,
            proxy_internal_port: 8000,
            proxy_metrics_port: 9090,
            nginx_http_port: 8081,
            cert_path: "/etc/bench/cert.pem".into(),
            key_path: "/etc/bench/key.pem".into(),
            host_cert_path: "/tmp/cert.pem".into(),
            host_key_path: "/tmp/key.pem".into(),
        }
    }

    #[test]
    fn substitutes_simple() {
        assert_eq!(
            render("hello {{ proxy_host }}", &ctx()),
            "hello bench.local"
        );
    }

    #[test]
    fn allows_whitespace() {
        assert_eq!(render("{{proxy_host}}={{ proxy_host }}", &ctx()), "bench.local=bench.local");
    }

    #[test]
    fn leaves_unknown_intact() {
        assert_eq!(
            render("{{ proxy_host }} {{ bogus }}", &ctx()),
            "bench.local {{ bogus }}"
        );
    }

    #[test]
    fn handles_unmatched_open() {
        assert_eq!(render("foo {{ bar", &ctx()), "foo {{ bar");
    }
}
