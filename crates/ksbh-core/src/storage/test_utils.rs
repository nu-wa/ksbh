use crate::storage::module_session_key::ModuleSessionKey;
#[cfg(feature = "test-util")]
use crate::storage::redis_hashmap::RedisHashMap;
use tokio::time::Duration;

pub struct MockSessionStore {
    inner: RedisHashMap<ModuleSessionKey, Vec<u8>>,
}

impl MockSessionStore {
    pub fn new() -> Self {
        Self {
            inner: RedisHashMap::new(Some(Duration::from_secs(3600)), None, None),
        }
    }

    pub async fn get(&self, key: &ModuleSessionKey) -> Option<Vec<u8>> {
        self.inner.get_hot(key).await.map(|entry| entry.inner())
    }

    pub async fn insert(&self, key: ModuleSessionKey, value: Vec<u8>, _ttl: u64) {
        let _ = self.inner.upsert(key, value).await;
    }

    pub fn inner(&self) -> &RedisHashMap<ModuleSessionKey, Vec<u8>> {
        &self.inner
    }
}

impl Default for MockSessionStore {
    fn default() -> Self {
        Self::new()
    }
}
