pub struct DynamicTLS {
    certs: ksbh_core::certs::CertsReader,
}

impl DynamicTLS {
    pub fn new(certs: ksbh_core::certs::CertsReader) -> Self {
        Self { certs }
    }
}

static DEFAULT_CERT: ::std::sync::LazyLock<Option<ksbh_core::certs::Certificate>> =
    ::std::sync::LazyLock::new(load_default_cert);

fn load_default_cert() -> Option<ksbh_core::certs::Certificate> {
    let cert_file = ::std::env::var("KSBH__TLS__DEFAULT_CERT_FILE")
        .unwrap_or_else(|_| "/app/config/default-tls.crt".to_string());
    let key_file = ::std::env::var("KSBH__TLS__DEFAULT_KEY_FILE")
        .unwrap_or_else(|_| "/app/config/default-tls.key".to_string());

    let cert_pem = match ::std::fs::read(&cert_file) {
        Ok(v) => v,
        Err(err) => {
            tracing::error!(
                "failed to read fallback certificate file '{}': {}",
                cert_file,
                err
            );
            return None;
        }
    };
    let key_pem = match ::std::fs::read(&key_file) {
        Ok(v) => v,
        Err(err) => {
            tracing::error!("failed to read fallback key file '{}': {}", key_file, err);
            return None;
        }
    };

    let key = match pingora::tls::pkey::PKey::private_key_from_pem(&key_pem) {
        Ok(v) => v,
        Err(err) => {
            tracing::error!(
                "failed to parse fallback private key from '{}': {}",
                key_file,
                err
            );
            return None;
        }
    };
    let cert_chain = match pingora::tls::x509::X509::stack_from_pem(&cert_pem) {
        Ok(v) => v,
        Err(err) => {
            tracing::error!(
                "failed to parse fallback certificate chain from '{}': {}",
                cert_file,
                err
            );
            return None;
        }
    };
    if cert_chain.is_empty() {
        tracing::error!("fallback certificate chain from '{}' is empty", cert_file);
        return None;
    }

    Some((
        ::std::sync::Arc::new(key),
        ::std::sync::Arc::new(cert_chain),
    ))
}

#[async_trait::async_trait]
impl pingora::listeners::TlsAccept for DynamicTLS {
    async fn certificate_callback(&self, ssl: &mut pingora::tls::ssl::SslRef) {
        ssl.set_security_level(1);

        let sni = ssl
            .servername(pingora::tls::ssl::NameType::HOST_NAME)
            .map(|s| s.to_string());
        let cert_data = match sni.as_ref() {
            Some(s) => self.certs.get_cert(s).await,
            None => None,
        };

        let fallback_cert = DEFAULT_CERT.as_ref();
        let (private_key, cert_chain) = match cert_data.as_ref().or(fallback_cert) {
            Some(cert) => (cert.0.as_ref(), cert.1.as_ref()),
            None => {
                tracing::error!(
                    "no certificate available for TLS handshake (sni='{}')",
                    sni.as_deref().unwrap_or("unknown/no-sni")
                );
                return;
            }
        };

        let sni_str = sni.as_deref().unwrap_or("unknown/no-sni");

        if let Some(leaf) = cert_chain.first()
            && let Err(e) = pingora::tls::ext::ssl_use_certificate(ssl, leaf)
        {
            tracing::error!(
                "There was an error setting leaf certificate for {}, '{}'",
                sni_str,
                e
            );
            return;
        }

        if let Err(e) = pingora::tls::ext::ssl_use_private_key(ssl, private_key) {
            tracing::error!(
                "There was an error setting certificate for {}, '{}'",
                sni_str,
                e
            );
            return;
        }

        for intermediate in cert_chain.iter().skip(1) {
            if let Err(e) = ssl.add_chain_cert(intermediate.clone()) {
                tracing::error!(
                    "There was an error setting intermediate certificate for {}, '{}'",
                    sni_str,
                    e
                );
                return;
            };
        }
    }
}
