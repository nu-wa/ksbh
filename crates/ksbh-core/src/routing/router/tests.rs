#[cfg(test)]
mod tests {
    #[test]
    fn runtime_snapshot_tracks_modules_and_ingresses() {
        let (reader, writer) = crate::routing::Router::create();

        let mut module_config = hashbrown::HashMap::new();
        module_config.insert(
            ksbh_types::KsbhStr::new("content"),
            ksbh_types::KsbhStr::new("hello"),
        );

        writer.upsert_module(
            "robots-test",
            false,
            ::std::sync::Arc::new(module_config),
            crate::modules::ModuleConfigurationSpec {
                name: "robots-test".to_string(),
                r#type: crate::modules::ModuleConfigurationType::RobotsDotTXT,
                weight: 100,
                global: false,
                secret_ref: None,
                config: None,
                requires_body: false,
            },
        );

        let mut host_paths = crate::routing::HostPaths::default();
        host_paths.prefix.push((
            ksbh_types::KsbhStr::new("/"),
            crate::routing::ServiceBackendType::Static,
        ));

        writer.insert_ingress(
            "ingress-a",
            vec![(::std::sync::Arc::from("example.local"), host_paths)],
            crate::routing::IngressModuleConfig {
                modules: vec![::std::sync::Arc::from("robots-test")],
                excluded_modules: vec![],
            },
            None,
        );

        let snapshot = reader.snapshot_runtime_state();

        assert_eq!(snapshot.modules.len(), 1);
        assert_eq!(snapshot.modules[0].name, "robots-test");
        assert_eq!(snapshot.ingresses.len(), 1);
        assert_eq!(snapshot.ingresses[0].ingress_name, "ingress-a");
        assert_eq!(snapshot.ingresses[0].host, "example.local");
        assert_eq!(snapshot.ingresses[0].attached_modules, vec!["robots-test"]);
        assert_eq!(snapshot.ingresses[0].merged_modules, vec!["robots-test"]);
    }

    #[test]
    fn merged_modules_keep_global_and_ingress_scopes_separate() {
        let (_reader, writer) = crate::routing::Router::create();

        writer.upsert_module(
            "global-low",
            true,
            ::std::sync::Arc::new(hashbrown::HashMap::new()),
            crate::modules::ModuleConfigurationSpec {
                name: "global-low".to_string(),
                r#type: crate::modules::ModuleConfigurationType::Custom("global-low".to_string()),
                weight: 10,
                global: true,
                secret_ref: None,
                config: None,
                requires_body: false,
            },
        );
        writer.upsert_module(
            "global-high",
            true,
            ::std::sync::Arc::new(hashbrown::HashMap::new()),
            crate::modules::ModuleConfigurationSpec {
                name: "global-high".to_string(),
                r#type: crate::modules::ModuleConfigurationType::Custom("global-high".to_string()),
                weight: 100,
                global: true,
                secret_ref: None,
                config: None,
                requires_body: false,
            },
        );
        writer.upsert_module(
            "ingress-high",
            false,
            ::std::sync::Arc::new(hashbrown::HashMap::new()),
            crate::modules::ModuleConfigurationSpec {
                name: "ingress-high".to_string(),
                r#type: crate::modules::ModuleConfigurationType::Custom("ingress-high".to_string()),
                weight: 1000,
                global: false,
                secret_ref: None,
                config: None,
                requires_body: false,
            },
        );
        writer.upsert_module(
            "ingress-low",
            false,
            ::std::sync::Arc::new(hashbrown::HashMap::new()),
            crate::modules::ModuleConfigurationSpec {
                name: "ingress-low".to_string(),
                r#type: crate::modules::ModuleConfigurationType::Custom("ingress-low".to_string()),
                weight: 1,
                global: false,
                secret_ref: None,
                config: None,
                requires_body: false,
            },
        );

        let mut host_paths = crate::routing::HostPaths::default();
        host_paths.prefix.push((
            ksbh_types::KsbhStr::new("/"),
            crate::routing::ServiceBackendType::Static,
        ));

        writer.insert_ingress(
            "ingress-a",
            vec![(::std::sync::Arc::from("example.local"), host_paths)],
            crate::routing::IngressModuleConfig {
                modules: vec![
                    ::std::sync::Arc::from("ingress-low"),
                    ::std::sync::Arc::from("ingress-high"),
                ],
                excluded_modules: vec![],
            },
            None,
        );

        let modules = writer
            .router
            .get_ingress_modules(&::std::sync::Arc::from("ingress-a"));
        let module_names: Vec<_> = modules
            .iter()
            .map(|module| module.name.to_string())
            .collect();

        assert_eq!(
            module_names,
            vec!["global-high", "global-low", "ingress-high", "ingress-low"]
        );
    }

    #[test]
    fn equal_weights_are_tiebroken_by_name() {
        let (_reader, writer) = crate::routing::Router::create();

        writer.upsert_module(
            "beta",
            true,
            ::std::sync::Arc::new(hashbrown::HashMap::new()),
            crate::modules::ModuleConfigurationSpec {
                name: "beta".to_string(),
                r#type: crate::modules::ModuleConfigurationType::Custom("beta".to_string()),
                weight: 5,
                global: true,
                secret_ref: None,
                config: None,
                requires_body: false,
            },
        );
        writer.upsert_module(
            "alpha",
            true,
            ::std::sync::Arc::new(hashbrown::HashMap::new()),
            crate::modules::ModuleConfigurationSpec {
                name: "alpha".to_string(),
                r#type: crate::modules::ModuleConfigurationType::Custom("alpha".to_string()),
                weight: 5,
                global: true,
                secret_ref: None,
                config: None,
                requires_body: false,
            },
        );

        let modules = writer.router.get_global_modules();
        let module_names: Vec<_> = modules
            .iter()
            .map(|module| module.name.to_string())
            .collect();

        assert_eq!(module_names, vec!["alpha", "beta"]);
    }
}
