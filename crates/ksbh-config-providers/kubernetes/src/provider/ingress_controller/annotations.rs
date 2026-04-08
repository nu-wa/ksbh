pub struct Annotations {
    pub modules: Vec<::std::sync::Arc<str>>,
    pub excluded_modules: Vec<::std::sync::Arc<str>>,
    pub https: bool,
}

impl Annotations {
    pub fn new(
        annotations: Option<
            &std::collections::BTreeMap<::std::string::String, ::std::string::String>,
        >,
    ) -> Self {
        let mut modules = Vec::new();
        let mut excluded_modules = Vec::new();
        let mut https = false;

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

            if let Some(value) = annotations.get(ksbh_core::constants::KSBH_ANNOTATION_KEY_PROTCOL)
            {
                let mut value = value.clone().to_lowercase();
                ksbh_core::utils::remove_whitespace(&mut value);

                if value == "https" {
                    https = true;
                }
            }
        }

        Self {
            modules,
            excluded_modules,
            https,
        }
    }
}
