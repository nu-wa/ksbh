#[derive(Clone)]
pub struct ModuleMetric {
    pub(super) name: ksbh_types::KsbhStr,
    pub(super) exec_time: f64,
    pub(super) global: bool,
    pub(super) stage: super::RequestStage,
    pub(super) decision: Option<ksbh_types::prelude::ProxyDecision>,
}

impl ModuleMetric {
    pub fn new(
        name: &str,
        exec_time: f64,
        global: bool,
        stage: super::RequestStage,
        decision: Option<ksbh_types::prelude::ProxyDecision>,
    ) -> Self {
        Self {
            name: ksbh_types::KsbhStr::new(name),
            exec_time,
            global,
            stage,
            decision,
        }
    }

    pub fn new_early(
        name: &str,
        exec_time: f64,
        global: bool,
        decision: Option<ksbh_types::prelude::ProxyDecision>,
    ) -> Self {
        Self::new(
            name,
            exec_time,
            global,
            super::RequestStage::EarlyFilter,
            decision,
        )
    }

    pub fn new_request(
        name: &str,
        exec_time: f64,
        global: bool,
        decision: Option<ksbh_types::prelude::ProxyDecision>,
    ) -> Self {
        Self::new(
            name,
            exec_time,
            global,
            super::RequestStage::RequestFilter,
            decision,
        )
    }
}

impl ::std::fmt::Debug for ModuleMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Module<{}>: global: {}, decision: {:?}, time: {:.2}, stage: {:?}",
            self.name,
            self.global,
            self.decision,
            self.exec_time * 1000.0f64,
            self.stage
        )
    }
}
