pub mod module_session_key;
pub mod redis_hashmap;

pub const CONNECTION_RETRIES: u8 = 5;

/// Public interface for Redis connection providers.
pub trait RedisProvider: Send + Sync {
    fn get_connection(&self) -> Result<Box<dyn redis::ConnectionLike + Send>, redis::RedisError>;
}

impl RedisProvider for redis::Client {
    fn get_connection(&self) -> Result<Box<dyn redis::ConnectionLike + Send>, redis::RedisError> {
        let conn = self.get_connection()?;
        Ok(Box::new(conn))
    }
}

#[derive(Clone)]
pub struct Storage {
    redis_provider: Option<::std::sync::Arc<dyn RedisProvider>>,
}

#[cfg(feature = "test-util")]
pub struct MockProvider {
    pub mock: redis_test::MockRedisConnection,
}

#[cfg(feature = "test-util")]
impl RedisProvider for MockProvider {
    fn get_connection(&self) -> Result<Box<dyn redis::ConnectionLike + Send>, redis::RedisError> {
        Ok(Box::new(self.mock.clone()))
    }
}

#[cfg(feature = "test-util")]
impl MockProvider {
    pub fn new(commands: Vec<redis_test::MockCmd>) -> Self {
        Self {
            mock: redis_test::MockRedisConnection::new(commands),
        }
    }

    pub fn get_mut(&mut self) -> &mut redis_test::MockRedisConnection {
        &mut self.mock
    }
}

impl Storage {
    /// Creates a Storage instance with no Redis provider ( Redis operations will fail).
    pub fn empty() -> Self {
        Self {
            redis_provider: None,
        }
    }

    /// Creates a Storage instance with a Redis client provider.
    ///
    /// Retries connection up to 5 times with 5-second intervals.
    pub async fn new_with_redis_client_provider(
        redis_url: &str,
    ) -> Result<Self, Box<dyn ::std::error::Error>> {
        use redis::ConnectionLike;
        let mut redis_connection = None;
        let mut redis_err = None;
        let mut redis_attempt = 0;

        while redis_attempt < CONNECTION_RETRIES {
            match redis::Client::open(redis_url) {
                Ok(redis_conn) => {
                    redis_connection = Some(redis_conn);
                    redis_err = None;
                    break;
                }
                Err(e) => {
                    redis_err = Some(e);
                }
            };
            tracing::error!("Could not reach redis, retrying in 5...");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            redis_attempt += 1;
        }

        let mut redis_client = match redis_connection {
            Some(client) => client,
            None => {
                tracing::error!("Redis connection error: {:?}", redis_err);
                return Err("Could not connect to redis.".into());
            }
        };

        if !redis_client.check_connection() {
            return Err("Could not connect to redis.".into());
        }

        Ok(Self {
            redis_provider: Some(::std::sync::Arc::new(redis_client)),
        })
    }

    #[cfg(feature = "test-util")]
    pub async fn new_mock(redis_provider: ::std::sync::Arc<MockProvider>) -> Self {
        Self {
            redis_provider: Some(redis_provider),
        }
    }

    /// Returns a Redis connection if a provider is configured.
    pub fn get_redis(&self) -> Result<Box<dyn redis::ConnectionLike + Send>, redis::RedisError> {
        match &self.redis_provider {
            Some(provider) => provider.get_connection(),
            None => Err(redis::RedisError::from((
                redis::ErrorKind::Io,
                "Redis not configured",
            ))),
        }
    }
}

#[cfg(feature = "test-util")]
pub mod test_utils;
