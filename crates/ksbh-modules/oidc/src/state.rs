pub const MODULE_NAME: &str = "oidc";

#[derive(serde::Serialize, serde::Deserialize)]
pub struct OidcSessionData {
    pub flow: Option<OidcFlowState>,
    pub refresh_token: Option<String>,
    pub oidc_complete: Option<i64>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct OidcFlowState {
    pub nonce: String,
    pub pkce_verifier: String,
    pub redirect_to: String,
    pub csrf_token: String,
    pub time: i64,
}

impl Default for OidcSessionData {
    fn default() -> Self {
        Self {
            flow: None,
            refresh_token: None,
            oidc_complete: None,
        }
    }
}

pub fn load(ctx: &ksbh_modules_sdk::ModuleContext<'_>) -> Result<OidcSessionData, ksbh_modules_sdk::ModuleError> {
    let Some(bytes) = ctx.session_get(MODULE_NAME)? else {
        return Ok(OidcSessionData::default());
    };

    Ok(rmp_serde::from_slice(&bytes).unwrap_or_default())
}

pub fn save(
    ctx: &ksbh_modules_sdk::ModuleContext<'_>,
    data: &OidcSessionData,
    ttl_secs: u64,
) -> Result<(), ksbh_modules_sdk::ModuleError> {
    let bytes = rmp_serde::to_vec(data).map_err(ksbh_modules_sdk::ModuleError::critical)?;
    ctx.session_set(MODULE_NAME, &bytes, Some(ttl_secs))
}
