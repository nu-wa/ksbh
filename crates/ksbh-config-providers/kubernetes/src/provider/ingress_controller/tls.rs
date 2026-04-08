pub async fn parse_tls_configs(
    ctx: &super::IngressController,
    namespace: &str,
    tls_configs: Option<&Vec<k8s_openapi::api::networking::v1::IngressTLS>>,
) -> Result<(), crate::provider::controller_error::ControllerError> {
    let Some(tls_configs) = tls_configs else {
        return Ok(());
    };

    let secret_api =
        kube::Api::<k8s_openapi::api::core::v1::Secret>::namespaced(ctx.client.clone(), namespace);

    for tls in tls_configs {
        let Some(secret_name) = tls.secret_name.clone() else {
            continue;
        };

        let tls_secret = {
            let result = tokio::time::timeout(
                tokio::time::Duration::from_secs(3),
                secret_api.get(&secret_name),
            )
            .await;

            match result {
                Ok(Ok(secret)) => Some(secret),
                Ok(Err(e)) => {
                    tracing::warn!("Failed to get secret {}/{}: {}", namespace, secret_name, e);
                    None
                }
                Err(e) => {
                    tracing::error!(
                        "TIMEOUT getting secret: {}.{}, {}",
                        namespace,
                        secret_name,
                        e
                    );
                    None
                }
            }
        };

        let Some(secret) = tls_secret else {
            continue;
        };

        let (tls_crt, tls_key) = match parse_tls_secret(&secret, &secret_name) {
            Ok(pair) => pair,
            Err(e) => {
                tracing::warn!(
                    "Failed to parse TLS secret {}/{}: {}",
                    namespace,
                    secret_name,
                    e
                );
                continue;
            }
        };

        let (private_key, certs) = match load_certificate(&tls_crt, &tls_key, &secret_name) {
            Ok(pair) => pair,
            Err(e) => {
                tracing::warn!(
                    "Failed to load certificate from secret {}: {}",
                    secret_name,
                    e
                );
                continue;
            }
        };

        let (domains, wildcards) = ksbh_core::certs::extract_domains_from_cert(&certs);

        if let Err(e) = ctx
            .certs
            .add_cert(&secret_name, private_key, certs, domains, wildcards)
            .await
        {
            tracing::error!("Failed to add cert: {}", e);
        } else {
            tracing::info!("added cert");
        }
    }

    Ok(())
}

pub(crate) fn parse_tls_secret(
    secret: &k8s_openapi::api::core::v1::Secret,
    secret_name: &str,
) -> Result<(::std::string::String, ::std::string::String), ::std::string::String> {
    let tls_crt = secret
        .data
        .as_ref()
        .and_then(|m| m.get("tls.crt").map(|b| b.0.clone()))
        .or_else(|| {
            secret
                .string_data
                .as_ref()
                .and_then(|m| m.get("tls.crt").map(|s| s.clone().into_bytes()))
        })
        .ok_or_else(|| format!("tls.crt not in secret {}", secret_name))?;

    let tls_crt = ::std::string::String::from_utf8(tls_crt)
        .map_err(|e| format!("Failed to convert tls_crt to utf8 string {}", e))?;

    let tls_key = secret
        .data
        .as_ref()
        .and_then(|m| m.get("tls.key").map(|b| b.0.clone()))
        .or_else(|| {
            secret
                .string_data
                .as_ref()
                .and_then(|m| m.get("tls.key").map(|s| s.clone().into_bytes()))
        })
        .ok_or_else(|| format!("tls.key not in secret {}", secret_name))?;

    let tls_key = ::std::string::String::from_utf8(tls_key)
        .map_err(|e| format!("Failed to convert tls_key to utf8 string {}", e))?;

    Ok((tls_crt, tls_key))
}

pub(crate) fn load_certificate(
    tls_crt: &str,
    tls_key: &str,
    secret_name: &str,
) -> Result<
    (
        pingora::tls::pkey::PKey<pingora::tls::pkey::Private>,
        Vec<pingora::tls::x509::X509>,
    ),
    ::std::string::String,
> {
    let private_key = pingora::tls::pkey::PKey::private_key_from_pem(tls_key.as_bytes())
        .map_err(|e| format!("Error when getting secret {}; '{}'", secret_name, e))?;

    let certs = pingora::tls::x509::X509::stack_from_pem(tls_crt.as_bytes())
        .map_err(|e| format!("Error when getting secret {}; '{}'", secret_name, e))?;

    Ok((private_key, certs))
}
