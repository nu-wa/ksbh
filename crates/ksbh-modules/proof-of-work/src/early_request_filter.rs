impl crate::ModulePow {
    pub async fn mod_early_request_filter(
        &self,
        config: &ksbh_core::modules::ModuleConfigurationValues,
        session: &mut dyn ksbh_types::prelude::ProxyProviderSession,
        req_info: &mut ksbh_core::proxy::EarlyRequestInformation,
        _storage: &::std::sync::Arc<ksbh_core::Storage>,
    ) -> Result<bool, super::errors::ModulePOWError> {
        let module_path = format!(
            "{}/pow",
            req_info
                .config
                .modules_internal_path
                .as_str()
                .trim_end_matches("/")
        );
        let cfg = super::ModulePowConfig::new(config)?;
        let client_information = &req_info.client_information;
        let mut difficulty = cfg.difficulty;
        let mut multiplier = 0;
        let (hits_good, hits_bad) = self
            .metrics_w
            .get_http_hits(client_information)
            .await
            .map(|hits| (hits.good, hits.bad))
            .unwrap_or((1, 0));

        if hits_bad >= hits_good {
            difficulty += (hits_bad / 10) as usize;
            multiplier += (hits_bad / 10) as u64;
        }

        let now = chrono::Utc::now().naive_utc();
        static ONE_DAY: ::std::time::Duration = ::std::time::Duration::from_hours(24);

        let http_req_info = &req_info.http_request_info;
        let query = &http_req_info.query;
        let mut cookie = req_info.cookie.clone();

        let mut challenge_expired = false;

        // Do we even bother with the request ?
        let skip_request = {
            if query.path.as_str() == "/favicon.ico" {
                true
            } else if let Some(challenge_complete) = cookie.challenge_complete {
                if now > challenge_complete + ONE_DAY {
                    challenge_expired = true;
                    false
                } else {
                    true
                }
            } else {
                false
            }
        };

        if session.get_header(http::header::UPGRADE).is_some() && !skip_request {
            return Err(super::errors::ModulePOWError::Unauthorized);
        }

        if skip_request {
            return Ok(false);
        }

        let secret_slice: &[u8; 32] = match cfg.secret.as_bytes()[0..32].try_into() {
            Ok(slice) => slice,
            Err(e) => {
                tracing::warn!("Bad !! {:?}", e);

                return Err(e.into());
            }
        };

        // If it's a first request, create challenge and serve HTML with javascript to complete the hash,
        // also reset the cookie's challenge variable.
        if (http_req_info.method.0 == http::method::Method::GET
            && !query.path.as_str().starts_with(module_path.as_str()))
            || challenge_expired
        {
            // Redirect back to the original query
            let redirect_to = http_req_info.uri.to_string();

            // Send challenge html
            let iat = ::std::time::SystemTime::now()
                .duration_since(::std::time::UNIX_EPOCH)?
                .as_secs();

            // Create challenge
            let mut b3_hasher = blake3::Hasher::new_keyed(secret_slice);

            b3_hasher.update(client_information.ip.to_string().as_bytes());
            if let Some(ua) = &client_information.user_agent {
                b3_hasher.update(ua.to_string().as_bytes());
            }
            b3_hasher.update(iat.to_string().as_bytes());

            let signature = b3_hasher.finalize();
            let mut html_ctx = gingembre::Context::new();

            html_ctx.set("challenge", format!("{}.{}", iat, signature.to_hex()));
            html_ctx.set("difficulty", difficulty.to_string());
            html_ctx.set("url", module_path);
            html_ctx.set("redirect_to", redirect_to);

            cookie.challenge_complete = None;

            let html = bytes::Bytes::from(self.templates.render("base.html", &html_ctx).await?);

            session
                .write_response(
                    match http::Response::builder()
                        .status(http::StatusCode::UNAUTHORIZED)
                        .header(http::header::SET_COOKIE, cookie.to_cookie_header()?)
                        .header(http::header::CONTENT_LENGTH, html.len())
                        .body(html)
                    {
                        Ok(r) => r,
                        Err(e) => {
                            tracing::error!("Could not build response: '{e}'");
                            return Err(super::errors::ModulePOWError::InternalServerError(
                                "Could not build request".into(),
                            ));
                        }
                    },
                )
                .await?;

            return Ok(true);
        }

        if query.path.as_str().starts_with(module_path.as_str()) {
            if http_req_info.method.0 != http::method::Method::POST {
                return Err(super::errors::ModulePOWError::BadRequest(
                    "Invalid METHOD".into(),
                ));
            }

            let body_bytes = session.read_request_body().await?.ok_or(
                super::errors::ModulePOWError::BadRequest("Missing body".into()),
            )?;
            let body_str = String::from_utf8(body_bytes.to_vec())?;

            let mut challenge = String::new();
            let mut nonce: u64 = 0;
            let mut redirect_to = String::from("/");

            for part in body_str.split('&') {
                let mut kv = part.splitn(2, '=');
                let key = kv.next().unwrap_or("");
                let val = kv.next().unwrap_or("");

                let decoded_val = urlencoding::decode(val)?.into_owned();

                match key {
                    "challenge" => challenge = decoded_val,
                    "nonce" => nonce = decoded_val.parse().unwrap_or(0),
                    "redirect_to" => redirect_to = decoded_val,
                    _ => {}
                }
            }

            if challenge.is_empty() || nonce == 0 {
                return Err(super::errors::ModulePOWError::BadRequest(
                    "Invalid Form Data".into(),
                ));
            }

            let parts: Vec<&str> = challenge.split(".").collect();

            if parts.len() != 2 {
                return Err(super::errors::ModulePOWError::BadRequest(
                    "Invalid Form Data".into(),
                ));
            }

            let (iat, user_submitted_signature) = (parts[0], parts[1]);

            let mut b3_hasher = blake3::Hasher::new_keyed(secret_slice);

            b3_hasher.update(client_information.ip.to_string().as_bytes());
            if let Some(ua) = &client_information.user_agent {
                b3_hasher.update(ua.to_string().as_bytes());
            }
            b3_hasher.update(iat.to_string().as_bytes());

            let real_signature = b3_hasher.finalize();
            if real_signature.to_hex().as_str() != user_submitted_signature {
                return Err(super::errors::ModulePOWError::BadRequest("Invalid".into()));
            }

            let iat: u64 = iat.parse().unwrap_or(0);
            let now = ::std::time::SystemTime::now()
                .duration_since(::std::time::UNIX_EPOCH)?
                .as_secs();

            // User failed to complete the challenge in time
            if now > iat + (300 + (60 * multiplier)) {
                return Err(super::errors::ModulePOWError::BadRequest("Invalid".into()));
            }

            use sha2::Digest;
            let mut sha = sha2::Sha256::new();
            sha.update(format!("{}{}", challenge, nonce));
            let hash = hex::encode(sha.finalize());

            if !hash.starts_with(&"0".repeat(difficulty)) {
                return Err(super::errors::ModulePOWError::BadRequest("Invalid".into()));
            }

            cookie.challenge_complete = Some(chrono::Utc::now().naive_utc());

            session
                .write_response(
                    match http::Response::builder()
                        .status(http::StatusCode::FOUND)
                        .header(http::header::LOCATION, redirect_to)
                        .header(http::header::SET_COOKIE, cookie.to_cookie_header()?)
                        .header(http::header::CONTENT_LENGTH, 0)
                        .body(bytes::Bytes::new())
                    {
                        Ok(r) => r,
                        Err(e) => {
                            tracing::error!("Could not build response: '{e}'");
                            return Err(super::errors::ModulePOWError::InternalServerError(
                                "Could not build request".into(),
                            ));
                        }
                    },
                )
                .await?;

            self.metrics_w.good_boy(client_information.clone()).await;

            return Ok(true);
        }
        Ok(false)
    }
}
