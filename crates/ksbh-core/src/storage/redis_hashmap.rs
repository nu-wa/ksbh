#[derive(Clone)]
pub struct RedisHashMap<K, V> {
    ttl: tokio::time::Duration,
    redis_ttl: tokio::time::Duration,
    container: ::std::sync::Arc<scc::HashMap<K, StoredValue<V>>>,
    redis_connection: Option<::std::sync::Arc<super::Storage>>,
}

#[derive(Debug, Clone)]
pub struct StoredValue<V> {
    expires: tokio::time::Instant,
    value: V,
}

impl<K, V> ::std::fmt::Debug for RedisHashMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ttl: {:?}, redis_ttl: {:?}", self.ttl, self.redis_ttl)
    }
}

impl<V: Clone> StoredValue<V> {
    pub fn inner(&self) -> V {
        self.value.clone()
    }
}

impl<K, V> RedisHashMap<K, V>
where
    K: serde::de::DeserializeOwned
        + serde::Serialize
        + ::std::cmp::Eq
        + ::std::hash::Hash
        + Clone
        + Send
        + Sync
        + 'static
        + ::std::fmt::Debug,
    V: serde::de::DeserializeOwned
        + serde::Serialize
        + Clone
        + Send
        + Sync
        + 'static
        + ::std::fmt::Debug,
{
    pub fn new(
        ttl: Option<tokio::time::Duration>,
        redis_ttl: Option<tokio::time::Duration>,
        redis_connection: Option<::std::sync::Arc<super::Storage>>,
    ) -> Self {
        Self {
            ttl: ttl.unwrap_or(tokio::time::Duration::from_hours(1)),
            redis_ttl: redis_ttl.unwrap_or(tokio::time::Duration::from_hours(24)),
            redis_connection,
            container: ::std::sync::Arc::new(scc::HashMap::new()),
        }
    }

    pub async fn get_hot(
        &self,
        key: &K,
    ) -> Option<scc::hash_map::OccupiedEntry<'_, K, StoredValue<V>>> {
        let now = tokio::time::Instant::now();

        if let Some(mut v) = self.container.get_async(key).await
            && v.expires > now
        {
            v.expires = now + self.ttl;
            return Some(v);
        }

        self.container.remove_sync(key);
        None
    }

    pub async fn get_cold(
        &self,
        key: K,
    ) -> Option<scc::hash_map::OccupiedEntry<'_, K, StoredValue<V>>> {
        let now = tokio::time::Instant::now();

        if let Some(mut v) = self.container.get_async(&key).await
            && v.expires > now
        {
            v.expires = now + self.ttl;
            return Some(v);
        }

        self.container.remove_sync(&key);

        if let Some(redis) = self.redis_connection.clone()
            && let Ok(mut conn) = redis.get_redis().await
        {
            let key_bytes = rmp_serde::to_vec(&key).ok()?;
            let raw_data: Option<Vec<u8>> = redis::cmd("GET")
                .arg(&key_bytes)
                .query::<Option<Vec<u8>>>(&mut *conn)
                .ok()
                .flatten();

            if let Some(bytes) = raw_data
                && let Ok(decoded) = rmp_serde::from_slice::<V>(&bytes)
            {
                let value = StoredValue {
                    expires: tokio::time::Instant::now() + self.ttl,
                    value: decoded,
                };
                self.container.upsert_async(key.clone(), value).await;

                return self.container.get_async(&key).await;
            }
        }

        None
    }

    pub async fn upsert(&self, key: K, value: V) -> Option<StoredValue<V>> {
        self.container
            .upsert_async(
                key,
                StoredValue {
                    value,
                    expires: tokio::time::Instant::now() + self.ttl,
                },
            )
            .await
    }

    pub fn get_sync(&self, key: &K) -> Option<V> {
        let now = tokio::time::Instant::now();

        if let Some(v) = self.container.get_sync(key) {
            if v.expires > now {
                return Some(v.value.clone());
            }
            self.container.remove_sync(key);
        }
        None
    }

    pub fn set_sync(&self, key: K, value: V) -> Option<V> {
        self.container
            .upsert_sync(
                key,
                StoredValue {
                    value,
                    expires: tokio::time::Instant::now() + self.ttl,
                },
            )
            .map(|old| old.value)
    }

    pub fn set_with_ttl_sync(&self, key: K, value: V, ttl_secs: u64) -> Option<V> {
        let ttl = tokio::time::Duration::from_secs(ttl_secs);
        self.container
            .upsert_sync(
                key,
                StoredValue {
                    value,
                    expires: tokio::time::Instant::now() + ttl,
                },
            )
            .map(|old| old.value)
    }

    pub fn remove_sync(&self, key: &K) {
        self.container.remove_sync(key);
    }

    pub fn get_redis_sync(&self, key: &K) -> Option<V> {
        let redis = self.redis_connection.as_ref()?;
        let rt = tokio::runtime::Handle::current();

        rt.block_on(async {
            let mut conn = redis.get_redis().await.ok()?;
            let key_bytes = rmp_serde::to_vec(key).ok()?;
            let raw_data: Option<Vec<u8>> = redis::cmd("GET")
                .arg(&key_bytes)
                .query::<Option<Vec<u8>>>(&mut *conn)
                .ok()
                .flatten();

            if let Some(bytes) = raw_data {
                rmp_serde::from_slice::<V>(&bytes).ok()
            } else {
                None
            }
        })
    }

    pub fn set_redis_sync(&self, key: K, value: V) -> bool {
        let redis = match self.redis_connection.as_ref() {
            Some(r) => r,
            None => return false,
        };
        let rt = tokio::runtime::Handle::current();
        let redis_ttl = self.redis_ttl.as_secs();

        rt.block_on(async {
            let mut conn = match redis.get_redis().await {
                Ok(c) => c,
                Err(_) => return false,
            };
            let key_bytes = match rmp_serde::to_vec(&key) {
                Ok(k) => k,
                Err(_) => return false,
            };
            let encoded = match rmp_serde::to_vec(&value) {
                Ok(v) => v,
                Err(_) => return false,
            };
            redis::cmd("SETEX")
                .arg(&key_bytes)
                .arg(redis_ttl)
                .arg(&encoded)
                .query::<()>(&mut *conn)
                .is_ok()
        })
    }

    pub fn set_redis_sync_with_ttl(&self, key: K, value: V, ttl_secs: u64) -> bool {
        let redis = match self.redis_connection.as_ref() {
            Some(r) => r,
            None => return false,
        };
        let rt = tokio::runtime::Handle::current();

        rt.block_on(async {
            let mut conn = match redis.get_redis().await {
                Ok(c) => c,
                Err(_) => return false,
            };
            let key_bytes = match rmp_serde::to_vec(&key) {
                Ok(k) => k,
                Err(_) => return false,
            };
            let encoded = match rmp_serde::to_vec(&value) {
                Ok(v) => v,
                Err(_) => return false,
            };
            redis::cmd("SETEX")
                .arg(&key_bytes)
                .arg(ttl_secs)
                .arg(&encoded)
                .query::<()>(&mut *conn)
                .is_ok()
        })
    }

    pub fn remove_redis_sync(&self, key: &K) -> bool {
        let redis = match self.redis_connection.as_ref() {
            Some(r) => r,
            None => return false,
        };
        let rt = tokio::runtime::Handle::current();

        rt.block_on(async {
            let mut conn = match redis.get_redis().await {
                Ok(c) => c,
                Err(_) => return false,
            };
            let key_bytes = match rmp_serde::to_vec(key) {
                Ok(k) => k,
                Err(_) => return false,
            };
            redis::cmd("DEL")
                .arg(&key_bytes)
                .query::<i32>(&mut *conn)
                .map(|n| n > 0)
                .unwrap_or(false)
        })
    }

    pub async fn watch(&self, interval: tokio::time::Duration) -> tokio::task::JoinHandle<()> {
        let redis_conn = self.redis_connection.clone();
        let redis_ttl = self.redis_ttl;
        let container = self.container.clone();

        tokio::task::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);

            loop {
                interval_timer.tick().await;

                if let Some(redis) = redis_conn.clone()
                    && let Ok(mut conn) = redis.get_redis().await
                {
                    let now = tokio::time::Instant::now();
                    let mut to_store = Vec::new();

                    container.retain_sync(|key, value| {
                        if value.expires <= now {
                            to_store.push((key.clone(), value.value.clone()));
                            false
                        } else {
                            true
                        }
                    });

                    if !to_store.is_empty() {
                        tracing::debug!("storing: {:?}", to_store);
                        for (k, v) in to_store {
                            if let Ok(encoded) = rmp_serde::to_vec(&v)
                                && let Ok(encoded_key) = rmp_serde::to_vec(&k)
                                && let Err(e) = redis::cmd("SETEX")
                                    .arg(&encoded_key)
                                    .arg(redis_ttl.as_secs())
                                    .arg(encoded)
                                    .query::<()>(&mut *conn)
                            {
                                tracing::error!("Error when redis and shit: {:?}", e);
                            }
                        }
                    }
                } else {
                    tracing::warn!("Lost connection to redis ...");
                }
            }
        })
    }
}

impl<V> ::std::borrow::Borrow<V> for StoredValue<V> {
    fn borrow(&self) -> &V {
        &self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn test_basic() {
        use ksbh_types::KsbhStr;

        let key = KsbhStr::new("key");
        let key_as_bytes = rmp_serde::to_vec(&key).unwrap();
        let value = KsbhStr::new("value");
        let encoded = rmp_serde::to_vec(&value).unwrap();

        let mock_provider = crate::storage::MockProvider::new(vec![
            redis_test::MockCmd::new(
                redis::cmd("SETEX")
                    .arg(&key_as_bytes)
                    .arg(500u64)
                    .arg(&encoded),
                Ok("OK"),
            ),
            redis_test::MockCmd::new(redis::cmd("GET").arg(&key_as_bytes), Ok(encoded)),
            redis_test::MockCmd::new(redis::cmd("GET").arg(&key_as_bytes), Ok(redis::Value::Nil)),
        ]);

        let ttl = Some(tokio::time::Duration::from_millis(1000));
        let redis_ttl = Some(tokio::time::Duration::from_millis(500000));
        let storage = ::std::sync::Arc::new(
            crate::storage::Storage::new_mock(::std::sync::Arc::new(mock_provider)).await,
        );

        let redis_hashmap: RedisHashMap<KsbhStr, KsbhStr> =
            RedisHashMap::new(ttl, redis_ttl, Some(storage));

        let watch_handle = redis_hashmap
            .watch(tokio::time::Duration::from_millis(10))
            .await;

        let key = KsbhStr::new("key");
        let value = KsbhStr::new("value");

        redis_hashmap.upsert(key.clone(), value.clone()).await;

        assert!(redis_hashmap.get_hot(&key).await.is_some());
        assert!(redis_hashmap.get_cold(key.clone()).await.is_some());

        tokio::time::advance(tokio::time::Duration::from_millis(2500)).await;
        for _ in 0..5 {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            tokio::task::yield_now().await;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        assert!(redis_hashmap.get_hot(&key).await.is_none());
        assert!(redis_hashmap.get_cold(key.clone()).await.is_some());

        tokio::time::advance(tokio::time::Duration::from_secs(510)).await;

        assert!(
            redis_hashmap.get_cold(key.clone()).await.is_none(),
            "Should be expired in Redis now"
        );
        watch_handle.abort();
    }
}
