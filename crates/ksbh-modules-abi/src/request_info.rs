use crate::types::*;

#[repr(C)]
pub struct RequestInfo {
    pub uri: KSBHString,
    pub host: KSBHString,
    pub method: KSBHString,
    pub path: KSBHString,
    pub query_params: KSBHSliceKVStrings,
    pub scheme: KSBHString,
    pub port: u16,
    pub is_ws_handshake: u8,
}
