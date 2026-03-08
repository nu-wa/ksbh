#[derive(Clone)]
pub(crate) struct Templates {
    engine: ::std::sync::Arc<tokio::sync::RwLock<Engine>>,
}

unsafe impl Send for Templates {}
unsafe impl Sync for Templates {}

#[derive(Debug)]
pub enum TemplateError {
    RwLock(String),
    RenderError(String),
}

impl ::std::error::Error for TemplateError {}

impl ::std::fmt::Display for TemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TemplateError: {}",
            match self {
                Self::RwLock(m) => m,
                Self::RenderError(m) => m,
            }
        )
    }
}

struct Engine {
    inner: gingembre::Engine<gingembre::InMemoryLoader>,
}

impl Engine {
    fn new(loader: gingembre::InMemoryLoader) -> Self {
        Self {
            inner: gingembre::Engine::new(loader),
        }
    }

    async fn render(&mut self, name: &str, ctx: &gingembre::Context) -> Result<String, String> {
        match self.inner.render(name, ctx).await {
            Ok(r) => Ok(r),
            Err(e) => {
                tracing::debug!("{:?}", e);
                Err(e.to_string())
            }
        }
    }
}

impl ::std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TemplateEngine")
    }
}

impl ::std::fmt::Debug for Templates {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Templates")
    }
}

impl Templates {
    pub fn new(loader: gingembre::InMemoryLoader) -> Self {
        Self {
            engine: ::std::sync::Arc::new(tokio::sync::RwLock::new(Engine::new(loader))),
        }
    }

    pub async fn render(
        &self,
        name: &str,
        ctx: &gingembre::Context,
    ) -> Result<String, TemplateError> {
        let mut engine = self.engine.write().await;

        match engine.render(name, ctx).await {
            Ok(string) => Ok(string),
            Err(err) => {
                tracing::error!("Template error: {}", err);

                Err(TemplateError::RenderError(err))
            }
        }
    }
}

impl From<::std::sync::PoisonError<tokio::sync::RwLockWriteGuard<'_, Engine>>> for TemplateError {
    fn from(value: ::std::sync::PoisonError<tokio::sync::RwLockWriteGuard<'_, Engine>>) -> Self {
        Self::RwLock(value.to_string())
    }
}
