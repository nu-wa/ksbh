//! A very naive attempt at keeping some values in memory and storing them into Redis after memory TTL expires.

/// Hot/cold cache with Redis persistence.
///
/// - Hot cache: in-memory with TTL (default 1 hour)
/// - Cold cache: Redis with separate TTL (default 24 hours)
#[derive(Clone)]
pub struct RedisHashMap<K, V> {
    ttl: tokio::time::Duration,
    redis_ttl: tokio::time::Duration,
    container: ::std::sync::Arc<scc::HashMap<K, StoredValue<V>>>,
    redis_connection: Option<::std::sync::Arc<super::Storage>>,
}

/// A value stored in the hot cache with expiration time.
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
    /// Creates a new RedisHashMap.
    ///
    /// - `ttl`: Hot cache TTL (defaults to 1 hour)
    /// - `redis_ttl`: Cold cache TTL for Redis persistence (defaults to 24 hours)
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

    pub fn get_hot_sync(&self, key: &K) -> Option<V> {
        let now = tokio::time::Instant::now();

        if let Some(v) = self.container.get_sync(key) {
            if v.expires > now {
                return Some(v.value.clone());
            }
            self.container.remove_sync(key);
        }
        None
    }

    /// Synchronously checks hot cache first, then cold (Redis) cache.
    ///
    /// If found in cold cache, promotes value to hot cache.
    pub fn get_hot_or_cold_sync(&self, key: &K) -> Option<V> {
        if let Some(v) = self.get_hot_sync(key) {
            return Some(v);
        }

        if let Some(v) = self.get_redis_sync(key) {
            self.set_sync(key.clone(), v.clone());
            return Some(v);
        }

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
            && let Ok(mut conn) = redis.get_redis()
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

        let mut conn = redis.get_redis().ok()?;
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
    }

    pub fn set_redis_sync(&self, key: K, value: V) -> bool {
        self.set_redis_sync_ref(&key, &value)
    }

    pub fn set_redis_sync_ref(&self, key: &K, value: &V) -> bool {
        let redis = match self.redis_connection.as_ref() {
            Some(r) => r,
            None => return false,
        };
        let redis_ttl = self.redis_ttl.as_secs();

        let mut conn = match redis.get_redis() {
            Ok(c) => c,
            Err(_) => return false,
        };
        let key_bytes = match rmp_serde::to_vec(key) {
            Ok(k) => k,
            Err(_) => return false,
        };
        let encoded = match rmp_serde::to_vec(value) {
            Ok(v) => v,
            Err(_) => return false,
        };
        redis::cmd("SETEX")
            .arg(&key_bytes)
            .arg(redis_ttl)
            .arg(&encoded)
            .query::<()>(&mut *conn)
            .is_ok()
    }

    pub fn set_redis_sync_with_ttl(&self, key: K, value: V, ttl_secs: u64) -> bool {
        self.set_redis_sync_with_ttl_ref(&key, &value, ttl_secs)
    }

    pub fn set_redis_sync_with_ttl_ref(&self, key: &K, value: &V, ttl_secs: u64) -> bool {
        let redis = match self.redis_connection.as_ref() {
            Some(r) => r,
            None => return false,
        };

        let mut conn = match redis.get_redis() {
            Ok(c) => c,
            Err(_) => return false,
        };
        let key_bytes = match rmp_serde::to_vec(key) {
            Ok(k) => k,
            Err(_) => return false,
        };
        let encoded = match rmp_serde::to_vec(value) {
            Ok(v) => v,
            Err(_) => return false,
        };
        redis::cmd("SETEX")
            .arg(&key_bytes)
            .arg(ttl_secs)
            .arg(&encoded)
            .query::<()>(&mut *conn)
            .is_ok()
    }

    pub fn remove_redis_sync(&self, key: &K) -> bool {
        let redis = match self.redis_connection.as_ref() {
            Some(r) => r,
            None => return false,
        };

        let mut conn = match redis.get_redis() {
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
    }

    fn incr_hash_impl(
        key_bytes: &[u8],
        field: &str,
        delta: i64,
        conn: &mut Box<dyn redis::ConnectionLike + Send>,
    ) -> bool {
        redis::cmd("HINCRBY")
            .arg(key_bytes)
            .arg(field)
            .arg(delta)
            .query::<i64>(conn.as_mut())
            .is_ok()
    }

    pub fn incr_hash_sync(&self, key: &K, field: &str, delta: i64) -> bool {
        let key_bytes = match rmp_serde::to_vec(key) {
            Ok(k) => k,
            Err(_) => return false,
        };
        self.incr_hash_by_key_bytes(&key_bytes, field, delta)
    }

    pub fn incr_hash_by_key_bytes_sync(&self, key_bytes: &[u8], field: &str, delta: i64) -> bool {
        self.incr_hash_by_key_bytes(key_bytes, field, delta)
    }

    fn incr_hash_by_key_bytes(&self, key_bytes: &[u8], field: &str, delta: i64) -> bool {
        let redis = match self.redis_connection.as_ref() {
            Some(r) => r,
            None => return false,
        };

        let mut conn = match redis.get_redis() {
            Ok(c) => c,
            Err(_) => return false,
        };

        if Self::incr_hash_impl(key_bytes, field, delta, &mut conn) {
            return true;
        }

        let mut conn = match redis.get_redis() {
            Ok(c) => c,
            Err(_) => return false,
        };
        Self::incr_hash_impl(key_bytes, field, delta, &mut conn)
    }

    pub fn get_hash_field_sync(&self, key: &K, field: &str) -> Option<u32> {
        let key_bytes = match rmp_serde::to_vec(key) {
            Ok(k) => k,
            Err(_) => return None,
        };
        self.get_hash_field_by_key_bytes_sync(&key_bytes, field)
    }

    pub fn incr_hash_async(&self, key: K, field: &'static str, delta: i64) {
        let redis = match self.redis_connection.clone() {
            Some(r) => r,
            None => return,
        };

        let key_for_task = match rmp_serde::to_vec(&key) {
            Ok(k) => k,
            Err(_) => return,
        };

        let field_static = field;
        let redis_for_retry = redis.clone();
        let key_for_retry = key_for_task.clone();

        tokio::spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                let mut conn = match redis.get_redis() {
                    Ok(c) => c,
                    Err(_) => return Err(()),
                };

                let res = redis::cmd("HINCRBY")
                    .arg(&key_for_task)
                    .arg(field_static)
                    .arg(delta)
                    .query::<i64>(&mut *conn);

                match res {
                    Ok(_) => Ok(()),
                    Err(_) => Err(()),
                }
            })
            .await;

            if result.is_err() || result.unwrap().is_err() {
                tokio::spawn(async move {
                    let _ = tokio::task::spawn_blocking(move || {
                        let mut conn = match redis_for_retry.get_redis() {
                            Ok(c) => c,
                            Err(_) => return,
                        };
                        let _ = redis::cmd("HINCRBY")
                            .arg(&key_for_retry)
                            .arg(field_static)
                            .arg(delta)
                            .query::<i64>(&mut *conn);
                    })
                    .await;
                });
            }
        });
    }

    pub fn get_hash_all_sync(&self, key: &K) -> Option<(u32, u32)> {
        let redis = self.redis_connection.as_ref()?;

        let mut conn = redis.get_redis().ok()?;
        let key_bytes = rmp_serde::to_vec(key).ok()?;

        let result: Option<Vec<(Vec<u8>, i64)>> =
            redis::cmd("HGETALL").arg(&key_bytes).query(&mut *conn).ok();

        match result {
            Some(items) => {
                let mut good = 0u32;
                let mut bad = 0u32;
                for (field, value) in items {
                    let field_str = String::from_utf8_lossy(&field);
                    match field_str.as_ref() {
                        "good" => good = value as u32,
                        "bad" => bad = value as u32,
                        _ => {}
                    }
                }
                if good > 0 || bad > 0 {
                    Some((good, bad))
                } else {
                    None
                }
            }
            None => None,
        }
    }

    pub fn get_hash_field_by_key_bytes_sync(&self, key_bytes: &[u8], field: &str) -> Option<u32> {
        let redis = self.redis_connection.as_ref()?;

        let mut conn = redis.get_redis().ok()?;

        redis::cmd("HGET")
            .arg(key_bytes)
            .arg(field)
            .query::<Option<i64>>(&mut *conn)
            .ok()
            .flatten()
            .map(|v| v as u32)
    }

    pub fn get_hash_all_by_key_bytes_sync(&self, key_bytes: &[u8]) -> Option<(u32, u32)> {
        let redis = self.redis_connection.as_ref()?;

        let mut conn = redis.get_redis().ok()?;

        let result: Option<Vec<(Vec<u8>, i64)>> =
            redis::cmd("HGETALL").arg(key_bytes).query(&mut *conn).ok();

        match result {
            Some(items) => {
                let mut good = 0u32;
                let mut bad = 0u32;
                for (field, value) in items {
                    let field_str = String::from_utf8_lossy(&field);
                    match field_str.as_ref() {
                        "good" => good = value as u32,
                        "bad" => bad = value as u32,
                        _ => {}
                    }
                }
                if good > 0 || bad > 0 {
                    Some((good, bad))
                } else {
                    None
                }
            }
            None => None,
        }
    }

    /// Spawns a background task that periodically persists expired hot values to Redis.
    ///
    /// Runs until the returned JoinHandle is dropped.
    pub async fn watch(&self, interval: tokio::time::Duration) -> tokio::task::JoinHandle<()> {
        let redis_conn = self.redis_connection.clone();
        let redis_ttl = self.redis_ttl;
        let container = self.container.clone();

        tokio::task::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);

            loop {
                interval_timer.tick().await;

                if let Some(redis) = redis_conn.clone()
                    && let Ok(mut conn) = redis.get_redis()
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
