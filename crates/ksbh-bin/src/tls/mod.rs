pub struct DynamicTLS {
    certs: ksbh_core::certs::CertsReader,
}

impl DynamicTLS {
    pub fn new(certs: ksbh_core::certs::CertsReader) -> Self {
        Self { certs }
    }
}

static DEFAULT_CERT: ::std::sync::LazyLock<ksbh_core::certs::Certificate> =
    ::std::sync::LazyLock::new(|| {
        let key =
            pingora::tls::pkey::PKey::generate_ed25519().expect("Failed to generate ED25519 key");
        let mut builder =
            pingora::tls::x509::X509::builder().expect("Failed to create X509 builder");
        builder.set_version(2).expect("Failed to set X509 version");
        builder.set_pubkey(&key).expect("Failed to set public key");

        builder
            .sign(&key, pingora::tls::hash::MessageDigest::null())
            .expect("Failed to sign certificate");
        let crt = builder.build();

        (::std::sync::Arc::new(key), ::std::sync::Arc::new(vec![crt]))
    });

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

        let (private_key, cert_chain) = match cert_data {
            Some(ref cert) => (cert.0.as_ref(), cert.1.as_ref()),
            None => (DEFAULT_CERT.0.as_ref(), DEFAULT_CERT.1.as_ref()),
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
