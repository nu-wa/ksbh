pub mod certs;
pub mod config;
pub mod config_provider;
pub mod constants;
pub mod cookies;
pub mod metrics;
pub mod modules;
pub mod proxy;
pub mod routing;
pub mod storage;
pub mod utils;

pub use config::Config;
pub use notify;
pub use walkdir;

pub use cookie;

pub use storage::{RedisProvider, Storage, redis_hashmap::RedisHashMap};

pub static COOKIE_ENC_KEY: ::std::sync::LazyLock<crate::cookie::Key> =
    ::std::sync::LazyLock::new(|| {
        match crate::cookie::Key::try_from(
            match crate::utils::get_env_prefer_file(crate::constants::ENV_KSBH_COOKIE_KEY) {
                Ok(key) => key,
                Err(e) => {
                    tracing::warn!(
                        "{} environment variable not set generating random cookie",
                        e
                    );
                    return crate::cookie::Key::generate();
                }
            }
            .as_bytes(),
        ) {
            Ok(key) => key,
            Err(_) => crate::cookie::Key::generate(),
        }
    });

pub static COOKIE_NAME: ::std::sync::LazyLock<String> = ::std::sync::LazyLock::new(|| {
    match crate::utils::get_env_prefer_file(crate::constants::ENV_KSBH_SESSION_COOKIE_NAME) {
        Ok(name) => name,
        Err(_) => "ksbh".into(),
    }
});
