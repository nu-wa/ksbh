//! ksbh - A kubernetes first reverse proxy
//!
//! `ksbh` is a Kubernetes first reverse proxy built on [`pingora`](https://github.com/cloudflare/pingora).
//!

#[cfg(feature = "profiling")]
mod profiling;

mod apps;
mod proxy;
mod server;
mod services;
mod tls;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(all(feature = "profiling", target_env = "gnu"))]
#[unsafe(export_name = "malloc_conf")]
#[allow(non_upper_case_globals)]
pub static malloc_conf: &[u8] = b"prof:true,prof_active:true,lg_prof_sample:19\0";

fn main() -> anyhow::Result<()> {
    let (non_blocking, _guard) = tracing_appender::non_blocking(::std::io::stdout());
    tracing_log::LogTracer::init().ok();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_target(true) // prints the host crate/module target
                .compact(),
        )
        .with(tracing_subscriber::EnvFilter::from_env("DEBUG_LEVEL"))
        .try_init()
        .ok();

    tracing::info!("Starting ksbh...");

    let config = ksbh_core::Config::load().unwrap();

    tracing::debug!("Configuration: {:?}", config);

    #[cfg(feature = "profiling")]
    let _agent = profiling::create_pyroscope_agent(&config);

    // Cheat our way out of not being able to run async code
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

    let storage = rt.block_on(async {
        match &config.redis_url {
            Some(url) => ::std::sync::Arc::new(
                ksbh_core::Storage::new_with_redis_client_provider(url)
                    .await
                    .expect("Failed to create storage"),
            ),
            None => ::std::sync::Arc::new(ksbh_core::Storage::empty()),
        }
    });

    let _ = &*ksbh_core::metrics::prom::HTTP_REQUESTS_TOTAL;
    let _ = &*ksbh_core::metrics::prom::PINGORA_ERRORS_TOTAL;
    let _ = &*ksbh_core::metrics::prom::HTTP_RESPONSE_TIME_SECONDS;
    let _ = &*ksbh_core::metrics::prom::PLUGIN_EXEC_TIME;
    let _ = &*ksbh_core::metrics::prom::MODULE_EXEC_TIME;
    server::start_pingora(config, storage, _guard)
}
