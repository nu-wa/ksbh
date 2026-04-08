pub struct Annotations {
    pub modules: Vec<::std::sync::Arc<str>>,
    pub excluded_modules: Vec<::std::sync::Arc<str>>,
    pub peer_options: Option<ksbh_types::providers::proxy::peer_options::PeerOptions>,
}

#[derive(Debug)]
pub enum AnnotationError {
    Parsing(String),
}

impl ::std::fmt::Display for AnnotationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AnnotationError: {}",
            match self {
                Self::Parsing(e) => e.to_string(),
            }
        )
    }
}

impl ::std::error::Error for AnnotationError {}

impl From<::std::str::ParseBoolError> for AnnotationError {
    fn from(value: ::std::str::ParseBoolError) -> Self {
        Self::Parsing(value.to_string())
    }
}

impl Annotations {
    pub fn new(
        annotations: Option<
            &std::collections::BTreeMap<::std::string::String, ::std::string::String>,
        >,
    ) -> Result<Self, AnnotationError> {
        let mut modules = Vec::new();
        let mut excluded_modules = Vec::new();
        let mut peer_options = None;
        let mut verify_cert = true;
        let mut sni = None;
        let mut altnerative_names: Vec<std::sync::Arc<str>> = Vec::new();

        if let Some(annotations) = annotations {
            if let Some(value) = annotations.get(ksbh_core::constants::KSBH_ANNOTATION_KEY_MODULES)
            {
                let mut value = value.clone();
                ksbh_core::utils::remove_whitespace(&mut value);

                for module in value.split(",") {
                    modules.push(::std::sync::Arc::from(module));
                }
            }

            if let Some(value) =
                annotations.get(ksbh_core::constants::KSBH_ANNOTATION_KEY_EXCLUDED_MODULES)
            {
                let mut value = value.clone();
                ksbh_core::utils::remove_whitespace(&mut value);

                for excluded_module in value.split(",") {
                    excluded_modules.push(::std::sync::Arc::from(excluded_module));
                }
            }

            if let Some(value) =
                annotations.get(ksbh_core::constants::KSBH_ANNOTATION_KEY_MTLS_SKIP_CHECK_CERT)
            {
                let mut value = value.clone().to_lowercase();
                ksbh_core::utils::remove_whitespace(&mut value);

                verify_cert = value.parse()?;
            }

            if let Some(value) =
                annotations.get(ksbh_core::constants::KSBH_ANNOTATION_KEY_MTLS_CERT_SNI)
            {
                let mut value = value.clone();
                ksbh_core::utils::remove_whitespace(&mut value);

                sni = Some(::std::sync::Arc::from(value.as_str()));
            }

            if let Some(value) = annotations
                .get(ksbh_core::constants::KSBH_ANNOTATION_KEY_MTLS_CERT_ALTERNATIVE_NAMES)
            {
                let mut value = value.clone();
                ksbh_core::utils::remove_whitespace(&mut value);

                for alternative_name in value.split(",") {
                    altnerative_names.push(::std::sync::Arc::from(alternative_name));
                }
            }

            if let Some(value) = annotations.get(ksbh_core::constants::KSBH_ANNOTATION_KEY_MTLS) {
                let mut value = value.clone().to_lowercase();
                ksbh_core::utils::remove_whitespace(&mut value);

                let mtls_enabled: bool = value.parse()?;

                if mtls_enabled {
                    peer_options = Some(ksbh_types::providers::proxy::peer_options::PeerOptions {
                        sni,
                        verify_cert,
                        altnerative_names,
                    });
                }
            }
        }

        Ok(Self {
            modules,
            excluded_modules,
            peer_options,
        })
    }
}
