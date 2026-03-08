pub const REDIS_STATS_KEY: &str = "KSBH_STATS";
pub const PLUGIN_ENTRYPOINT: &str = "request_filter";
pub const PROXY_HEADER_NAME: &str = "Server";
pub const PROXY_HEADER_VALUE: &str = "ksbh";
pub const PLUGIN_TIMEOUT: ::std::time::Duration = ::std::time::Duration::from_secs(1);
pub const REDIS_MODULE_STORAGE_KEY: &str = "ksbh_modules";
pub const INGRESS_CLASS_NAME: &str = "ksbh";

pub const ENV_KSBH_COOKIE_KEY: &str = "KSBH_COOKIE_KEY";
pub const ENV_KSBH_SESSION_COOKIE_NAME: &str = "KSBH_SESSION_COOKIE_NAME";
pub const ENV_JWT_PEM_DECODE: &str = "KSBH_JWT_PEM_DEC_KEY";
pub const ENV_JWT_PEM_ENCODE: &str = "KSBH_JWT_PEM_ENC_KEY";

pub const KSBH_SERVICE_RESSOURCE_KIND: &str = "module";
pub const KSBH_SERVICE_RESSOURCE_KIND_STATIC: &str = "static";
pub const KSBH_SERVICE_RESSOURCE_KIND_SELF: &str = "self";
pub const KSBH_FINALIZER: &str = "ksbh.app/finalizer";

pub const KSBH_ANNOTATION_KEY_MODULES: &str = "ksbh.rs/modules";
pub const KSBH_ANNOTATION_KEY_PLUGINS: &str = "ksbh.rs/plugins";
pub const KSBH_K8S_SERVICE_RESSOURCE_API_GROUP: &str = "service-resource.ksbh.rs";
