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

use ::std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[cfg(all(feature = "profiling", target_env = "gnu"))]
#[unsafe(export_name = "malloc_conf")]
#[allow(non_upper_case_globals)]
pub static malloc_conf: &[u8] = b"prof:true,prof_active:true,lg_prof_sample:19\0";

pub static JWT_ENC_ENC_KEY: ::std::sync::LazyLock<jsonwebtoken::EncodingKey> =
    ::std::sync::LazyLock::new(|| {
        let key_content =
            match ksbh_core::utils::get_env_prefer_file(ksbh_core::constants::ENV_JWT_PEM_ENCODE) {
                Ok(key_content) => key_content,
                Err(e) => {
                    panic!("{e}");
                }
            };

        let key_content = key_content.trim();

        jsonwebtoken::EncodingKey::from_ec_pem(key_content.as_bytes()).unwrap()
    });

pub static JWT_ENC_DEC_KEY: ::std::sync::LazyLock<jsonwebtoken::DecodingKey> =
    ::std::sync::LazyLock::new(|| {
        let key_content =
            ksbh_core::utils::get_env_prefer_file(ksbh_core::constants::ENV_JWT_PEM_DECODE)
                .unwrap();
        let key_content = key_content.trim();

        jsonwebtoken::DecodingKey::from_ec_pem(key_content.as_bytes()).unwrap()
    });

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

    let config = ksbh_core::Config::build_from_env();

    tracing::debug!("Configuration: {:?}", config);

    #[cfg(feature = "profiling")]
    let _agent = profiling::create_pyroscope_agent(&config);

    let public_config = config.to_public();

    // Cheat our way out of not being able to run async code
    let rt = tokio::runtime::Runtime::new().expect("Error when setting up ksbh");

    let storage = rt.block_on(async {
        Arc::new(
            ksbh_core::Storage::new_with_redis_client_provider(&config.redis_url)
                .await
                .expect("Could not create storage"),
        )
    });
    let kube_client = rt.block_on(async {
        kube::Client::try_default()
            .await
            .expect("Could not connect to kubernetes")
    });

    let _ = &*ksbh_core::metrics::prom::HTTP_REQUESTS_TOTAL;
    let _ = &*ksbh_core::metrics::prom::PINGORA_ERRORS_TOTAL;
    let _ = &*ksbh_core::metrics::prom::HTTP_RESPONSE_TIME_SECONDS;
    let _ = &*ksbh_core::metrics::prom::PLUGIN_EXEC_TIME;
    let _ = &*ksbh_core::metrics::prom::MODULE_EXEC_TIME;
    server::start_pingora(config, kube_client, storage, public_config)
}
