use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use ksbh_modules_abi::types::{KSBHHostCtxHandle, KSBHHostFnReturn};

const SHARED_SESSION_PREFIX: &str = "__ksbh_shared__";

/// Selects which session keyspace a value is read from or written to.
///
/// `PerModule` keys are namespaced by the calling module's name, so two
/// modules can store data under the same `data_key` without colliding.
/// `Shared` keys use a single host-wide prefix (`__ksbh_shared__:`) and are
/// visible to every module in the same session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Keyspace {
    PerModule,
    Shared,
}

pub(crate) static ACTIVE_CALLS: LazyLock<ActiveCallRegistry> = LazyLock::new(ActiveCallRegistry::new);

#[derive(Debug)]
pub(crate) struct ActiveModuleCall {
    pub(crate) module_name: String,
    pub(crate) session_id: [u8; 16],
    pub(crate) reputation_key: [u8; 32],
    pub(crate) client_ip: Option<IpAddr>,
    pub(crate) session_store: ::std::sync::Arc<
        crate::storage::redis_hashmap::RedisHashMap<
            crate::storage::module_session_key::ModuleSessionKey,
            Vec<u8>,
        >,
    >,
    session_buffers: Mutex<HashMap<usize, Vec<u8>>>,
}

impl ActiveModuleCall {
    pub(crate) fn new(
        module_name: String,
        session_id: [u8; 16],
        reputation_key: [u8; 32],
        client_ip: Option<IpAddr>,
        session_store: ::std::sync::Arc<
            crate::storage::redis_hashmap::RedisHashMap<
                crate::storage::module_session_key::ModuleSessionKey,
                Vec<u8>,
            >,
        >,
    ) -> Self {
        Self {
            module_name,
            session_id,
            reputation_key,
            client_ip,
            session_store,
            session_buffers: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn session_key(
        &self,
        data_key: &str,
        keyspace: Keyspace,
    ) -> crate::storage::module_session_key::ModuleSessionKey {
        match keyspace {
            Keyspace::PerModule => self.module_session_key(data_key),
            Keyspace::Shared => self.shared_session_key(data_key),
        }
    }

    fn module_session_key(&self, data_key: &str) -> crate::storage::module_session_key::ModuleSessionKey {
        let session_id = uuid::Uuid::from_bytes(self.session_id);
        crate::storage::module_session_key::ModuleSessionKey::new(
            &format!("{}:{}", self.module_name, data_key),
            session_id,
        )
    }

    fn shared_session_key(&self, data_key: &str) -> crate::storage::module_session_key::ModuleSessionKey {
        let session_id = uuid::Uuid::from_bytes(self.session_id);
        crate::storage::module_session_key::ModuleSessionKey::new(
            &format!("{}:{}", SHARED_SESSION_PREFIX, data_key),
            session_id,
        )
    }

    pub(crate) fn track_session_buffer(&self, ptr: *const u8, value: Vec<u8>) {
        let mut buffers = self
            .session_buffers
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        buffers.insert(ptr as usize, value);
    }

    pub(crate) fn free_session_buffer(&self, ptr: *const u8) {
        let mut buffers = self
            .session_buffers
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        buffers.remove(&(ptr as usize));
    }
}

pub(crate) struct ActiveCallRegistry {
    next_id: AtomicU64,
    calls: scc::HashMap<u64, Arc<ActiveModuleCall>>,
}

impl ActiveCallRegistry {
    pub(crate) fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            calls: scc::HashMap::new(),
        }
    }

    pub(crate) fn insert(&self, call: Arc<ActiveModuleCall>) -> u64 {
        loop {
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            if id == 0 {
                continue;
            }

            if self.calls.insert_sync(id, Arc::clone(&call)).is_ok() {
                return id;
            }
        }
    }

    fn get(&self, handle: KSBHHostCtxHandle) -> Option<Arc<ActiveModuleCall>> {
        self.calls
            .get_sync(&handle.inner)
            .map(|entry| Arc::clone(entry.get()))
    }

    fn remove(&self, id: u64) {
        self.calls.remove_sync(&id);
    }
}

pub(crate) struct ActiveCallGuard {
    ctx_handle: u64,
    finished: bool,
}

impl ActiveCallGuard {
    pub(crate) fn new(ctx_handle: u64) -> Self {
        Self {
            ctx_handle,
            finished: false,
        }
    }

    pub(crate) fn finish(&mut self) {
        if !self.finished {
            ACTIVE_CALLS.remove(self.ctx_handle);
            self.finished = true;
        }
    }
}

impl Drop for ActiveCallGuard {
    fn drop(&mut self) {
        self.finish();
    }
}

pub(crate) fn resolve_active_call(
    ctx_handle: KSBHHostCtxHandle,
) -> Result<Arc<ActiveModuleCall>, KSBHHostFnReturn> {
    ACTIVE_CALLS
        .get(ctx_handle)
        .ok_or(KSBHHostFnReturn::NotFound)
}
