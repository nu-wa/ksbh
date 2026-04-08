pub mod ingress;
pub mod modules;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct FileConfig {
    pub modules: Option<Vec<modules::FileConfigModules>>,
    pub ingresses: Vec<ingress::FileConfigIngress>,
}

#[derive(Debug)]
pub enum FileConfigError {
    Validation(&'static str),
    Config(config::ConfigError),
    Parsing(String),
}

impl ::std::error::Error for FileConfigError {}

impl ::std::fmt::Display for FileConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FileConfigError: {}",
            match self {
                Self::Config(e) => e.to_string(),
                Self::Parsing(e) => e.to_string(),
                Self::Validation(e) => e.to_string(),
            }
        )
    }
}

impl From<config::ConfigError> for FileConfigError {
    fn from(value: config::ConfigError) -> Self {
        Self::Config(value)
    }
}

impl From<Box<dyn ::std::error::Error + 'static>> for FileConfigError {
    fn from(value: Box<dyn ::std::error::Error + 'static>) -> Self {
        Self::Parsing(value.to_string())
    }
}

impl FileConfig {
    pub fn load(config_path: &::std::path::Path) -> Result<Self, FileConfigError> {
        let cfg = config::Config::builder()
            .add_source(config::File::with_name(config_path.to_str().unwrap()).required(true))
            .build()?;

        let cfg: Self = cfg.try_deserialize()?;

        cfg.validate()?;

        Ok(cfg)
    }

    fn validate(&self) -> Result<(), FileConfigError> {
        for ingress in &self.ingresses {
            if psl::suffix(ingress.host.as_bytes()).is_none() {
                return Err(FileConfigError::Validation("Invalid tld"));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_file() {
        let ctx = crate::test_utils::Context::new(
            "
modules:
  - name: 'some_module'
    global: false
    weight: 100
    type: robotsdottxt
    requires_body: false
  - name: 'some_other_module'
    weight: 200
    type: robotsdottxt
ingresses:
  - name: 'some_ingress'
    host: 'local.host'
    paths:
      - path: '/'
        type: 'prefix'
        backend: 'static'
        ",
        );

        let file_config = FileConfig::load(ctx.tmp_file.path()).unwrap();

        let modules = file_config.modules.unwrap();
        assert_eq!(modules.len(), 2);
        assert_eq!(modules[0].global, false);
        assert_eq!(
            modules[0].r#type,
            ksbh_core::modules::ModuleConfigurationType::RobotsDotTXT
        );
        assert_eq!(modules[1].global, false);
        assert_eq!(
            modules[1].r#type,
            ksbh_core::modules::ModuleConfigurationType::RobotsDotTXT
        );
        assert_eq!(&modules[1].name, "some_other_module");
    }
}
