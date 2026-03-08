pub mod module_session_key;
pub mod redis_hashmap;

pub const CONNECTION_RETRIES: u8 = 5;

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
    redis_provider: ::std::sync::Arc<dyn RedisProvider>,
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
            ::std::thread::sleep(tokio::time::Duration::from_secs(5));
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
            redis_provider: ::std::sync::Arc::new(redis_client),
        })
    }

    #[cfg(feature = "test-util")]
    pub async fn new_mock(redis_provider: ::std::sync::Arc<MockProvider>) -> Self {
        Self { redis_provider }
    }

    pub fn get_redis(&self) -> Result<Box<dyn redis::ConnectionLike + Send>, redis::RedisError> {
        self.redis_provider.get_connection()
    }
}

#[cfg(feature = "test-util")]
pub mod test_utils;
