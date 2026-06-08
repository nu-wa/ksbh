use ksbh_modules_abi::types::{
    KSBHBytes, KSBHKVString, KSBHModuleDecision, KSBHSliceKVStrings, KSBHString, ModuleResponse,
};

#[repr(C)]
pub struct SdkOwnedModuleResponse {
    // Important for abi to be first field so it can be converted back into abi type.
    abi: ksbh_modules_abi::types::ModuleResponse,
    body_owner: Option<Box<[u8]>>,
    header_entries: Box<[ksbh_modules_abi::types::KSBHKVString]>,
    header_key_owners: Vec<Box<[u8]>>,
    header_value_owners: Vec<Box<[u8]>>,
}

impl SdkOwnedModuleResponse {
    pub fn new(
        decision: KSBHModuleDecision,
        header_map: Option<http::HeaderMap>,
        body_owner: Option<Vec<u8>>,
        status_code: Option<http::StatusCode>,
    ) -> Self {
        let mut header_entries = vec![];
        let mut header_key_owners = vec![];
        let mut header_value_owners = vec![];

        if let Some(headers) = header_map {
            let mut last_name: Option<http::HeaderName> = None;

            for (maybe_name, value) in headers {
                let name = match maybe_name {
                    Some(name) => {
                        last_name = Some(name.clone());
                        name
                    }
                    None => match last_name.as_ref() {
                        Some(name) => name.clone(),
                        None => continue,
                    },
                };

                let key_box = name.as_str().as_bytes().to_vec().into_boxed_slice();
                let value_box = value.as_bytes().to_vec().into_boxed_slice();

                header_entries.push(KSBHKVString {
                    key: KSBHString {
                        inner: KSBHBytes {
                            ptr: key_box.as_ptr(),
                            len: key_box.len(),
                        },
                    },
                    value: KSBHString {
                        inner: KSBHBytes {
                            ptr: value_box.as_ptr(),
                            len: value_box.len(),
                        },
                    },
                });

                header_key_owners.push(key_box);
                header_value_owners.push(value_box);
            }
        }

        let header_entries = header_entries.into_boxed_slice();

        Self {
            abi: ModuleResponse {
                body: KSBHBytes {
                    ptr: body_owner.as_ref().map_or(std::ptr::null(), |b| b.as_ptr()),
                    len: body_owner.as_ref().map_or(0, |b| b.len()),
                },
                decision,
                headers: KSBHSliceKVStrings {
                    ptr: header_entries.as_ptr(),
                    len: header_entries.len(),
                },
                status_code: status_code.unwrap_or(http::StatusCode::OK).as_u16(),
            },
            body_owner: body_owner.map(|b| b.into_boxed_slice()),
            header_entries,
            header_key_owners,
            header_value_owners,
        }
    }

    pub fn new_empty(decision: Option<KSBHModuleDecision>) -> Self {
        Self::new(
            decision.unwrap_or(KSBHModuleDecision::Pass),
            None,
            None,
            None,
        )
    }

    pub fn from_http_response(
        decision: Option<KSBHModuleDecision>,
        http_response: Option<http::Response<bytes::Bytes>>,
    ) -> Self {
        if let Some(http_response) = http_response {
            Self::new(
                decision.unwrap_or(KSBHModuleDecision::Pass),
                Some(http_response.headers().to_owned()),
                Some(http_response.body().to_owned().to_vec()),
                Some(http_response.status()),
            )
        } else {
            Self::new_empty(decision)
        }
    }

    pub fn to_abi(self) -> *const ModuleResponse {
        let owned_ptr = Box::into_raw(Box::new(self));
        let abi_ptr = unsafe { &(*owned_ptr).abi as *const ModuleResponse };
        abi_ptr
    }
}

pub unsafe fn free_module_response_owned(ptr: *mut ModuleResponse) {
    if ptr.is_null() {
        return;
    }

    unsafe {
        let owned_ptr = ptr as *mut SdkOwnedModuleResponse;
        drop(Box::from_raw(owned_ptr));
    }
}
