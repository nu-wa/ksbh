//! Render a [`Scenario`] + [`RenderCtx`] into the three config files the
//! runner needs: a ksbh ingress YAML, an nginx config, and a vegeta targets
//! file.

use anyhow::Result;

use crate::scenario::Scenario;
use crate::template;

/// Inputs passed via CLI flags to `ksbh-bench render`.
///
/// `cert_path` / `key_path` are the in-container paths nginx sees
/// (typically `/etc/bench/cert.pem`). `host_cert_path` / `host_key_path`
/// are the corresponding host paths the ksbh process needs.
#[derive(Debug, Clone)]
pub struct RenderCtx {
    pub proxy_host: String,
    pub upstream_port: u16,
    pub proxy_http_port: u16,
    pub proxy_https_port: u16,
    pub proxy_internal_port: u16,
    pub proxy_metrics_port: u16,
    pub nginx_http_port: u16,
    pub cert_path: String,
    pub key_path: String,
    pub host_cert_path: String,
    pub host_key_path: String,
}

impl RenderCtx {
    /// Map a placeholder name to its string value.
    pub fn lookup(&self, name: &str) -> Option<String> {
        Some(match name {
            "proxy_host" => self.proxy_host.clone(),
            "upstream_port" => self.upstream_port.to_string(),
            "proxy_http_port" => self.proxy_http_port.to_string(),
            "proxy_https_port" => self.proxy_https_port.to_string(),
            "proxy_internal_port" => self.proxy_internal_port.to_string(),
            "proxy_metrics_port" => self.proxy_metrics_port.to_string(),
            "nginx_http_port" => self.nginx_http_port.to_string(),
            "cert_path" => self.cert_path.clone(),
            "key_path" => self.key_path.clone(),
            "host_cert_path" => self.host_cert_path.clone(),
            "host_key_path" => self.host_key_path.clone(),
            _ => return None,
        })
    }
}

/// The three output strings produced by a render.
#[derive(Debug, Clone)]
pub struct RenderOutput {
    pub ksbh_yaml: String,
    pub nginx_conf: String,
    pub vegeta_targets: String,
}

/// Render `scenario` against `ctx` into the three output strings.
pub fn render_scenario(scenario: &Scenario, ctx: &RenderCtx) -> Result<RenderOutput> {
    let ksbh_yaml = render_ksbh_yaml(ctx);
    let nginx_conf = render_nginx_conf(scenario, ctx);
    let vegeta_targets = render_vegeta_targets(scenario, ctx);

    Ok(RenderOutput {
        ksbh_yaml,
        nginx_conf,
        vegeta_targets,
    })
}

fn render_ksbh_yaml(ctx: &RenderCtx) -> String {
    format!(
        "ingresses:\n\
         \x20\x20- name: bench\n\
         \x20\x20\x20\x20host: {proxy_host}\n\
         \x20\x20\x20\x20tls:\n\
         \x20\x20\x20\x20\x20\x20cert_file: {cert}\n\
         \x20\x20\x20\x20\x20\x20key_file: {key}\n\
         \x20\x20\x20\x20paths:\n\
         \x20\x20\x20\x20\x20\x20- path: /\n\
         \x20\x20\x20\x20\x20\x20\x20\x20type: prefix\n\
         \x20\x20\x20\x20\x20\x20\x20\x20backend: service\n\
         \x20\x20\x20\x20\x20\x20\x20\x20service:\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20name: upstream\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20port: {port}\n",
        proxy_host = ctx.proxy_host,
        cert = ctx.host_cert_path,
        key = ctx.host_key_path,
        port = ctx.upstream_port,
    )
}

fn render_nginx_conf(scenario: &Scenario, ctx: &RenderCtx) -> String {
    let ovr = &scenario.nginx;
    let mut s = String::new();

    s.push_str("worker_processes 1;\n");
    s.push_str("events {\n");
    s.push_str("    worker_connections 65535;\n");
    s.push_str("}\n");
    s.push_str("http {\n");
    s.push_str("    access_log off;\n");
    s.push_str(&format!(
        "    client_max_body_size {};\n",
        ovr.client_max_body_size.as_deref().unwrap_or("10m")
    ));
    s.push_str(&format!(
        "    client_body_timeout {};\n",
        ovr.client_body_timeout.as_deref().unwrap_or("30s")
    ));
    s.push_str(&format!(
        "    client_header_timeout {};\n",
        ovr.client_header_timeout.as_deref().unwrap_or("30s")
    ));
    s.push_str(&format!(
        "    send_timeout {};\n",
        ovr.send_timeout.as_deref().unwrap_or("30s")
    ));
    s.push_str(&format!(
        "    keepalive_timeout {};\n",
        ovr.keepalive_timeout.as_deref().unwrap_or("60s")
    ));
    s.push_str(&format!(
        "    large_client_header_buffers {};\n",
        ovr.large_client_header_buffers
            .as_deref()
            .unwrap_or("4 8k")
    ));
    s.push_str("    gzip off;\n\n");

    // Primary HTTP server block.
    s.push_str("    server {\n");
    s.push_str(&format!("        listen {};\n", ctx.nginx_http_port));
    s.push_str("        server_name bench.local;\n\n");
    s.push_str("        location / {\n");
    s.push_str(&format!(
        "            proxy_pass http://127.0.0.1:{};\n",
        ctx.upstream_port
    ));
    s.push_str("            proxy_http_version 1.1;\n");
    s.push_str("            proxy_set_header Connection \"\";\n");
    s.push_str("            proxy_buffering off;\n");
    if let Some(buf) = &ovr.proxy_buffer_size {
        s.push_str(&format!("            proxy_buffer_size {};\n", buf));
    } else {
        s.push_str("            proxy_buffer_size 8k;\n");
    }
    if let Some(bufs) = &ovr.proxy_buffers {
        s.push_str(&format!("            proxy_buffers {};\n", bufs));
    } else {
        s.push_str("            proxy_buffers 4 8k;\n");
    }
    s.push_str("        }\n\n");
    s.push_str("        location = /__bench_healthz {\n");
    s.push_str("            stub_status;\n");
    s.push_str("            access_log off;\n");
    s.push_str("        }\n");
    s.push_str("    }\n");

    // Optional TLS server block.
    if scenario.scenario.tls {
        s.push_str("\n");
        s.push_str("    server {\n");
        s.push_str(&format!(
            "        listen {} ssl http2;\n",
            ctx.proxy_https_port
        ));
        s.push_str("        server_name bench.local;\n");
        s.push_str(&format!("        ssl_certificate {};\n", ctx.cert_path));
        s.push_str(&format!("        ssl_certificate_key {};\n", ctx.key_path));
        s.push_str("\n");
        s.push_str("        location / {\n");
        s.push_str(&format!(
            "            proxy_pass http://127.0.0.1:{};\n",
            ctx.upstream_port
        ));
        s.push_str("            proxy_http_version 1.1;\n");
        s.push_str("            proxy_set_header Connection \"\";\n");
        s.push_str("            proxy_buffering off;\n");
        s.push_str("        }\n");
        s.push_str("    }\n");
    }

    s.push_str("}\n");
    s
}

fn render_vegeta_targets(scenario: &Scenario, ctx: &RenderCtx) -> String {
    template::render(&scenario.vegeta.targets, ctx)
}
