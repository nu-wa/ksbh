//! Render a self-contained HTML trend page from `bench/history/*.json`.
//!
//! Each history file is either a single [`ResultDoc`] or an array; we
//! flatten, group by (scenario, proxy, metric), and emit a grid of cards,
//! each with an inline SVG sparkline.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::analyze::ResultDoc;

const METRICS: &[(&str, &str)] = &[
    ("rps", "RPS"),
    ("p50_ms", "p50 (ms)"),
    ("p99_ms", "p99 (ms)"),
    ("max_ms", "max (ms)"),
    ("proxy_peak_rss_kb", "peak RSS (KB)"),
];

pub fn run(history: &str, out: &str) -> Result<()> {
    let docs = load_all_history(history)?;
    let html = render_html(&docs);
    if let Some(parent) = Path::new(out).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating output dir {}", parent.display()))?;
        }
    }
    fs::write(out, html).with_context(|| format!("writing trend page to {}", out))?;
    Ok(())
}

pub fn load_all_history(dir: &str) -> Result<Vec<ResultDoc>> {
    let mut out = Vec::new();
    let entries = fs::read_dir(dir).with_context(|| format!("reading history dir {}", dir))?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let v: serde_json::Value = serde_json::from_str(&raw)
            .with_context(|| format!("parsing {}", path.display()))?;
        match v {
            serde_json::Value::Array(_) => {
                let docs: Vec<ResultDoc> = serde_json::from_value(v)
                    .with_context(|| format!("parsing array in {}", path.display()))?;
                out.extend(docs);
            }
            serde_json::Value::Object(_) => {
                let doc: ResultDoc = serde_json::from_value(v)
                    .with_context(|| format!("parsing object in {}", path.display()))?;
                out.push(doc);
            }
            _ => {}
        }
    }
    out.sort_by(|a, b| a.started_at.cmp(&b.started_at));
    Ok(out)
}

fn render_html(docs: &[ResultDoc]) -> String {
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n");
    html.push_str("<meta charset=\"utf-8\">\n");
    html.push_str("<title>ksbh benchmark trends</title>\n");
    html.push_str("<style>\n");
    html.push_str(CSS);
    html.push_str("</style>\n</head>\n<body>\n");
    html.push_str("<h1>ksbh benchmark trends</h1>\n");

    if docs.is_empty() {
        html.push_str("<p>No history files found.</p>\n");
    } else {
        html.push_str(&format!("<p>{} runs across {} scenarios.</p>\n", docs.len(), count_scenarios(docs)));
    }

    // Group by scenario, then by proxy.
    let mut by_scenario: BTreeMap<String, Vec<&ResultDoc>> = BTreeMap::new();
    for d in docs {
        by_scenario.entry(d.scenario.clone()).or_default().push(d);
    }

    for (scenario, scenario_docs) in &by_scenario {
        html.push_str(&format!("<h2>{}</h2>\n", esc(scenario)));
        // Group docs by proxy within the scenario.
        let mut by_proxy: BTreeMap<String, Vec<&&ResultDoc>> = BTreeMap::new();
        for d in scenario_docs {
            by_proxy.entry(d.proxy.clone()).or_default().push(d);
        }
        for (proxy, proxy_docs) in &by_proxy {
            html.push_str(&format!("<h3>{}</h3>\n", esc(proxy)));
            html.push_str("<div class=\"grid\">\n");
            for (metric_key, metric_label) in METRICS {
                let series: Vec<f64> = proxy_docs
                    .iter()
                    .map(|d| metric_value(d, metric_key))
                    .collect();
                html.push_str(&render_card(
                    metric_label,
                    &series,
                    proxy_docs.last().copied().unwrap(),
                ));
            }
            html.push_str("</div>\n");
        }
    }

    html.push_str("</body>\n</html>\n");
    html
}

fn count_scenarios(docs: &[ResultDoc]) -> usize {
    let mut s: std::collections::HashSet<String> = std::collections::HashSet::new();
    for d in docs {
        s.insert(d.scenario.clone());
    }
    s.len()
}

fn metric_value(d: &ResultDoc, key: &str) -> f64 {
    match key {
        "rps" => d.results.rps,
        "p50_ms" => d.results.p50_ms,
        "p99_ms" => d.results.p99_ms,
        "max_ms" => d.results.max_ms,
        "proxy_peak_rss_kb" => d.proxy_peak_rss_kb as f64,
        _ => 0.0,
    }
}

fn render_card(label: &str, series: &[f64], last_doc: &ResultDoc) -> String {
    let mut s = String::new();
    s.push_str("<div class=\"card\">\n");
    s.push_str(&format!("<div class=\"label\">{}</div>\n", esc(label)));

    if series.len() < 3 {
        s.push_str("<div class=\"placeholder\">needs more runs</div>\n");
    } else {
        s.push_str(&render_sparkline(series));
    }

    let last_value = series.last().copied().unwrap_or(0.0);
    s.push_str(&format!("<div class=\"latest\">{}</div>\n", fmt_num(last_value)));
    s.push_str(&format!(
        "<div class=\"date\">{}</div>\n",
        esc(&last_doc.started_at)
    ));
    s.push_str("</div>\n");
    s
}

fn render_sparkline(series: &[f64]) -> String {
    let w = 200.0;
    let h = 60.0;
    let pad = 4.0;

    if series.is_empty() {
        return format!("<svg width=\"{}\" height=\"{}\"></svg>", w, h);
    }

    let min = series.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = series.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let span = (max - min).max(f64::EPSILON);

    let step = if series.len() > 1 {
        (w - 2.0 * pad) / (series.len() - 1) as f64
    } else {
        0.0
    };

    let mut points = String::new();
    for (i, v) in series.iter().enumerate() {
        let x = pad + step * i as f64;
        let y = h - pad - (v - min) / span * (h - 2.0 * pad);
        if i > 0 {
            points.push(' ');
        }
        points.push_str(&format!("{:.2},{:.2}", x, y));
    }

    let mut circles = String::new();
    for (i, v) in series.iter().enumerate() {
        let x = pad + step * i as f64;
        let y = h - pad - (v - min) / span * (h - 2.0 * pad);
        circles.push_str(&format!(
            "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"1.5\"/>\n",
            x, y
        ));
    }

    format!(
        "<svg width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" class=\"sparkline\">\n\
         <polyline points=\"{points}\" fill=\"none\" stroke=\"#3a7bd5\" stroke-width=\"1.5\"/>\n\
         {circles}\
         <text x=\"{pad}\" y=\"{top_y}\" font-size=\"9\" fill=\"#888\">{max_lbl}</text>\n\
         <text x=\"{pad}\" y=\"{bot_y}\" font-size=\"9\" fill=\"#888\">{min_lbl}</text>\n\
         </svg>\n",
        w = w,
        h = h,
        pad = pad,
        points = points,
        circles = circles,
        top_y = 10.0,
        bot_y = h - 2.0,
        max_lbl = esc(&fmt_num(max)),
        min_lbl = esc(&fmt_num(min)),
    )
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

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

const CSS: &str = r#"
body { font-family: -apple-system, BlinkMacSystemFont, sans-serif; margin: 2em; color: #222; }
h1 { margin-bottom: 0.2em; }
h2 { border-bottom: 1px solid #ddd; padding-bottom: 0.2em; margin-top: 1.5em; }
h3 { color: #666; margin-bottom: 0.5em; }
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 1em; }
.card { border: 1px solid #ddd; border-radius: 6px; padding: 0.8em; background: #fafafa; }
.label { font-weight: 600; margin-bottom: 0.3em; }
.latest { font-size: 1.4em; font-family: monospace; margin-top: 0.3em; }
.date { font-size: 0.75em; color: #888; }
.placeholder { color: #999; font-style: italic; padding: 1em 0; text-align: center; }
.sparkline { display: block; margin: 0.3em 0; }
"#;
