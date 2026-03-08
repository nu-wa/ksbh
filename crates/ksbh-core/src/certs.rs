pub type Certificate = (
    ::std::sync::Arc<pingora::tls::pkey::PKey<pingora::tls::pkey::Private>>,
    ::std::sync::Arc<Vec<pingora::tls::x509::X509>>,
);

#[derive(Debug, Clone)]
pub struct Cert {
    private_key: ::std::sync::Arc<pingora::tls::pkey::PKey<pingora::tls::pkey::Private>>,
    certs: ::std::sync::Arc<Vec<pingora::tls::x509::X509>>,
    domains: Vec<ksbh_types::KsbhStr>,
    wildcards: Vec<ksbh_types::KsbhStr>,
}

#[derive(Debug)]
pub struct CertsRegistry {
    // Sorted by secret_name
    certs: scc::HashMap<ksbh_types::KsbhStr, ::std::sync::Arc<Cert>>,
    // Sorted by domain/sni
    certs_domains: scc::HashMap<::std::sync::Arc<str>, ::std::sync::Arc<Cert>>,
}

#[derive(Debug)]
pub struct CertsReader {
    registry: ::std::sync::Arc<CertsRegistry>,
}

#[derive(Debug, Clone)]
pub struct CertsWriter {
    registry: ::std::sync::Arc<CertsRegistry>,
}

impl Default for CertsRegistry {
    fn default() -> Self {
        Self {
            certs: scc::HashMap::new(),
            certs_domains: scc::HashMap::new(),
        }
    }
}

impl CertsRegistry {
    pub fn create() -> (CertsReader, CertsWriter) {
        let registry = ::std::sync::Arc::new(CertsRegistry::default());

        (
            CertsReader {
                registry: registry.clone(),
            },
            CertsWriter { registry },
        )
    }

    async fn add_cert(
        &self,
        name: &str,
        private_key: pingora::tls::pkey::PKey<pingora::tls::pkey::Private>,
        certs: Vec<pingora::tls::x509::X509>,
        domains: Vec<ksbh_types::KsbhStr>,
        wildcards: Vec<ksbh_types::KsbhStr>,
    ) -> Result<(), Box<dyn ::std::error::Error>> {
        let cert = ::std::sync::Arc::new(Cert {
            private_key: ::std::sync::Arc::new(private_key),
            certs: ::std::sync::Arc::new(certs),
            domains,
            wildcards,
        });

        self.certs
            .upsert_async(ksbh_types::KsbhStr::new(name), cert.clone())
            .await;

        for domain in &cert.domains {
            self.certs_domains
                .upsert_async(::std::sync::Arc::from(domain.as_str()), cert.clone())
                .await;
        }

        for wildcard in &cert.wildcards {
            if wildcard.len() > 2 {
                let suffix = &wildcard.as_str()[2..];
                self.certs_domains
                    .upsert_async(::std::sync::Arc::from(suffix), cert.clone())
                    .await;
            }
        }

        Ok(())
    }

    fn delete_cert(&self, name: &str) {
        self.certs.remove_sync(&ksbh_types::KsbhStr::new(name));
    }

    async fn get_cert(&self, domain_name: &str) -> Option<Certificate> {
        if let Some(cert) = self.certs_domains.get_async(domain_name).await {
            return Some((cert.private_key.clone(), cert.certs.clone()));
        }

        if let Some(pos) = domain_name.find('.') {
            let parent = &domain_name[pos + 1..];

            if let Some(cert) = self.certs_domains.get_async(parent).await {
                return Some((cert.private_key.clone(), cert.certs.clone()));
            }
        }

        let mut best: Option<(::std::sync::Arc<Cert>, usize)> = None;

        let mut it = self.certs.begin_async().await;

        while let Some(cert) = it {
            if cert
                .domains
                .iter()
                .any(|domain| domain.as_str() == domain_name)
            {
                return Some((cert.private_key.clone(), cert.certs.clone()));
            }

            for wc_domain in &cert.wildcards {
                if wc_domain.len() <= 2 {
                    continue;
                }
                let suffix = &wc_domain.as_str()[2..];
                if domain_name.ends_with(suffix) && domain_name.len() > suffix.len() {
                    match &best {
                        Some((_, best_len)) if *best_len >= suffix.len() => {}
                        _ => best = Some((cert.get().clone(), suffix.len())),
                    }
                }
            }

            it = cert.next_async().await;
        }

        best.map(|(c, _)| (c.private_key.clone(), c.certs.clone()))
    }
}

impl CertsWriter {
    pub async fn add_cert(
        &self,
        name: &str,
        private_key: pingora::tls::pkey::PKey<pingora::tls::pkey::Private>,
        certs: Vec<pingora::tls::x509::X509>,
        domains: Vec<ksbh_types::KsbhStr>,
        wildcards: Vec<ksbh_types::KsbhStr>,
    ) -> Result<(), Box<dyn ::std::error::Error>> {
        self.registry
            .add_cert(name, private_key, certs, domains, wildcards)
            .await
    }

    pub fn delete_cert(&self, name: &str) {
        self.registry.delete_cert(name);
    }
}

impl CertsReader {
    pub async fn get_cert(&self, name: &str) -> Option<Certificate> {
        self.registry.get_cert(name).await
    }
}

impl From<&::std::sync::Arc<CertsRegistry>> for CertsReader {
    fn from(value: &::std::sync::Arc<CertsRegistry>) -> Self {
        Self {
            registry: ::std::sync::Arc::clone(value),
        }
    }
}

impl From<&::std::sync::Arc<CertsRegistry>> for CertsWriter {
    fn from(value: &::std::sync::Arc<CertsRegistry>) -> Self {
        Self {
            registry: ::std::sync::Arc::clone(value),
        }
    }
}
