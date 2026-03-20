lazy_static::lazy_static! {
    pub static ref HTTP_REQUESTS_TOTAL: prometheus::CounterVec = prometheus ::register_counter_vec!(
        prometheus::opts!("ksbh_http_requests_total", "Total number of HTTP requests"),
        &["method", "status", "backend", "outcome", "host", "path"]
    ).expect("Failed to register HTTP_REQUESTS_TOTAL metric - metrics subsystem may be broken");

    pub static ref PINGORA_ERRORS_TOTAL: prometheus::CounterVec = prometheus::register_counter_vec!(
        prometheus::opts!("ksbh_pingora_errors_total", "Total number of Pingora internal errors"),
        &["error_type"]
    ).expect("Failed to register PINGORA_ERRORS_TOTAL metric - metrics subsystem may be broken");

    pub static ref HTTP_RESPONSE_TIME_SECONDS: prometheus::HistogramVec = prometheus::register_histogram_vec!(
        "ksbh_http_response_time_seconds",
        "The latency of the HTTP requests in seconds",
        &["backend", "status"],
        vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
    ).expect("Failed to register HTTP_RESPONSE_TIME_SECONDS metric - metrics subsystem may be broken");

    pub static ref MODULE_EXEC_TIME: prometheus::HistogramVec = prometheus::register_histogram_vec!(
        "ksbh_module_exec_time",
        "Execution time per plugin",
        &["module_name", "global", "sent_reply"],
        vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5]
    ).expect("Failed to register MODULE_EXEC_TIME metric - metrics subsystem may be broken");

    pub static ref PLUGIN_EXEC_TIME: prometheus::HistogramVec = prometheus::register_histogram_vec!(
        "ksbh_plugin_exec_time",
        "Execution time per plugin",
        &["plugin_name", "sent_reply"],
        vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5]
    ).expect("Failed to register PLUGIN_EXEC_TIME metric - metrics subsystem may be broken");
}
