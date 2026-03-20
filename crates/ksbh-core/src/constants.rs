pub const REDIS_STATS_KEY: &str = "KSBH_STATS";
pub const PLUGIN_ENTRYPOINT: &str = "request_filter";
pub const REDIS_MODULE_STORAGE_KEY: &str = "ksbh_modules";
pub const INGRESS_CLASS_NAME: &str = "ksbh";

pub const KSBH_SERVICE_RESSOURCE_KIND: &str = "module";
pub const KSBH_SERVICE_RESSOURCE_KIND_STATIC: &str = "static";
pub const KSBH_SERVICE_RESSOURCE_KIND_SELF: &str = "self";
pub const KSBH_FINALIZER: &str = "ksbh.app/finalizer";

pub const KSBH_ANNOTATION_KEY_MODULES: &str = "ksbh.rs/modules";
pub const KSBH_ANNOTATION_KEY_PLUGINS: &str = "ksbh.rs/plugins";
pub const KSBH_ANNOTATION_KEY_EXCLUDED_MODULES: &str = "ksbh.rs/excluded-modules";
pub const KSBH_K8S_SERVICE_RESSOURCE_API_GROUP: &str = "service-resource.ksbh.rs";
