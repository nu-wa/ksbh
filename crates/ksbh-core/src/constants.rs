pub const REDIS_STATS_KEY: &str = "KSBH_STATS";
pub const PLUGIN_ENTRYPOINT: &str = "request_filter";
pub const REDIS_MODULE_STORAGE_KEY: &str = "ksbh_modules";
pub const INGRESS_CLASS_NAME: &str = "ksbh";

pub const KSBH_SERVICE_RESSOURCE_KIND: &str = "module";
pub const KSBH_SERVICE_RESSOURCE_KIND_STATIC: &str = "static";
pub const KSBH_FINALIZER: &str = "ksbh.app/finalizer";

pub const KSBH_ANNOTATION_KEY_MODULES: &str = "ksbh.rs/modules";
pub const KSBH_ANNOTATION_KEY_PLUGINS: &str = "ksbh.rs/plugins";
pub const KSBH_ANNOTATION_KEY_MTLS: &str = "ksbh.rs/mtls";
pub const KSBH_ANNOTATION_KEY_MTLS_SKIP_CHECK_CERT: &str = "ksbh.rs/mtls/verify-cert";
pub const KSBH_ANNOTATION_KEY_MTLS_CERT_SNI: &str = "ksbh.rs/mtls/sni";
pub const KSBH_ANNOTATION_KEY_MTLS_CERT_ALTERNATIVE_NAMES: &str = "ksbh.rs/mtls/alternative-names";
pub const KSBH_ANNOTATION_KEY_EXCLUDED_MODULES: &str = "ksbh.rs/excluded-modules";
pub const KSBH_K8S_SERVICE_RESSOURCE_API_GROUP: &str = "service-resource.ksbh.rs";

pub const HEADER_X_FORWARDED_PROTO: &str = "X-Forwarded-Proto";
pub const HEADER_X_FORWARDED_SSL: &str = "X-Forwarded-Ssl";
pub const HEADER_X_FORWARDED_FOR: &str = "X-Forwarded-For";
pub const HEADER_X_FORWARDED_HOST: &str = "X-Forwarded-Host";
pub const HEADER_X_FORWARDED_PORT: &str = "X-Forwarded-Port";
pub const HEADER_X_REAL_IP: &str = "X-Real-IP";
pub const HEADER_FORWARDED: &str = "Forwarded";
pub const HEADER_X_KSBH_WS_DOWNSTREAM_TRANSPORT: &str = "x-ksbh-ws-downstream-transport";
