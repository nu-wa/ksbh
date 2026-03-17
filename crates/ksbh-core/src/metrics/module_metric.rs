pub struct ModuleMetric {
    pub(super) name: ksbh_types::KsbhStr,
    pub(super) exec_time: f64,
    pub(super) global: bool,
    pub(super) module_replied: bool,
}

impl Clone for ModuleMetric {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            exec_time: self.exec_time,
            global: self.global,
            module_replied: self.module_replied,
        }
    }
}

impl ModuleMetric {
    pub fn new(name: &str, exec_time: f64, global: bool, module_replied: bool) -> Self {
        Self {
            name: ksbh_types::KsbhStr::new(name),
            exec_time,
            global,
            module_replied,
        }
    }

    pub fn new_early(name: &str, exec_time: f64, global: bool, module_replied: bool) -> Self {
        Self::new(name, exec_time, global, module_replied)
    }

    pub fn new_request(name: &str, exec_time: f64, global: bool, module_replied: bool) -> Self {
        Self::new(name, exec_time, global, module_replied)
    }
}

impl ::std::fmt::Debug for ModuleMetric {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        write!(
            f,
            "Module<{}>: global: {}, module_replied: {}, time: {:.2}",
            self.name,
            self.global,
            self.module_replied,
            self.exec_time * 1000.0f64,
        )
    }
}
