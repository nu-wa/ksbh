pub mod metrics;
// pub mod proxy;

pub struct BackgroundService {
    config: ::std::sync::Arc<ksbh_core::config::Config>,
    sessions: ::std::sync::Arc<
        ksbh_core::storage::redis_hashmap::RedisHashMap<
            ksbh_core::storage::module_session_key::ModuleSessionKey,
            Vec<u8>,
        >,
    >,
    modules_libraries: ::std::sync::Arc<ksbh_core::modules::abi::module_host::ModuleHost>,
}

impl BackgroundService {
    pub fn new(
        modules_libraries: ::std::sync::Arc<ksbh_core::modules::abi::module_host::ModuleHost>,
        config: ::std::sync::Arc<ksbh_core::config::Config>,
        sessions: ::std::sync::Arc<
            ksbh_core::storage::redis_hashmap::RedisHashMap<
                ksbh_core::storage::module_session_key::ModuleSessionKey,
                Vec<u8>,
            >,
        >,
    ) -> Self {
        Self {
            config,
            sessions,
            modules_libraries,
        }
    }
}

#[async_trait::async_trait]
impl pingora::services::background::BackgroundService for BackgroundService {
    async fn start(&self, mut shutdown: pingora::server::ShutdownWatch) {
        tracing::info!("Starting background service");

        let (shutdown_signal, _) = tokio::sync::watch::channel(false);
        let plugins_shutdown_signal = shutdown.clone();
        let shutdown_signal = ::std::sync::Arc::new(shutdown_signal);

        let sessions = self.sessions.clone();
        let sessions_task_handle = tokio::task::spawn(async move {
            sessions.watch(tokio::time::Duration::from_hours(2)).await
        });

        let modules_library = self.modules_libraries.clone();
        let dir = self.config.modules_directory.clone();

        tracing::info!("Watching modules directory: {:?}", dir);

        let plugins_watch_task_handle = tokio::task::spawn(async move {
            let modules_library = modules_library.clone();

            ksbh_core::utils::watch_directory_files_async(
                dir,
                {
                    let modules_library = modules_library.clone();

                    move |entry: ksbh_core::walkdir::DirEntry| {
                        let modules_library = modules_library.clone();

                        async move {
                            // Skip directories, only process files
                            if !entry.file_type().is_file() {
                                tracing::trace!("Skipping non-file entry: {:?}", entry.path());
                                return;
                            }

                            let Some(name) = entry.path().file_stem() else {
                                tracing::warn!("Module path has no file stem: {:?}", entry.path());
                                return;
                            };

                            let Some(name) = name.to_str() else {
                                tracing::warn!(
                                    "Module path is not valid UTF-8: {:?}",
                                    entry.path()
                                );
                                return;
                            };

                            tracing::info!("Attempting to load module: {}", name);

                            if let Err(e) = modules_library.load_module(entry.path()) {
                                tracing::error!("Failed to load module {}; error: {}", name, e);
                            } else {
                                tracing::info!("Loaded module: {}", name);
                            }
                        }
                    }
                },
                async |_entry: ksbh_core::notify::Event| {},
                Some(plugins_shutdown_signal),
            )
            .await
        });

        tokio::select! {
            _ = shutdown.changed() => {
                if let Err(e) = shutdown_signal.send(true) {
                    tracing::error!("Could not send shutdown signal to applications {e}");
                }
            }
        };

        sessions_task_handle.abort();
        plugins_watch_task_handle.abort();

        tracing::info!("Ended background service");
    }
}
