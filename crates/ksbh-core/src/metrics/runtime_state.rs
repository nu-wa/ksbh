pub fn observe_runtime_snapshot(snapshot: &crate::routing::RuntimeStateSnapshot) {
    let mut host_names: ::std::collections::BTreeSet<&str> = ::std::collections::BTreeSet::new();
    let mut ingress_names: ::std::collections::BTreeSet<&str> = ::std::collections::BTreeSet::new();

    for ingress in &snapshot.ingresses {
        host_names.insert(ingress.host.as_str());
        ingress_names.insert(ingress.ingress_name.as_str());
    }

    crate::metrics::prom::RUNTIME_ACTIVE_INGRESSES.set(ingress_names.len() as i64);
    crate::metrics::prom::RUNTIME_ACTIVE_HOSTS.set(host_names.len() as i64);

    let global_modules = snapshot
        .modules
        .iter()
        .filter(|module| module.global)
        .count();
    let non_global_modules = snapshot.modules.len().saturating_sub(global_modules);

    crate::metrics::prom::RUNTIME_ACTIVE_GLOBAL_MODULES.set(global_modules as i64);
    crate::metrics::prom::RUNTIME_ACTIVE_NON_GLOBAL_MODULES.set(non_global_modules as i64);
}
