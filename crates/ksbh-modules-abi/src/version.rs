pub const KSBH_MAGIC: u64 = 0x4B_53_42_48_5F_41_42_49;

pub const KSBH_ABI_MAJOR: u16 = 0;
pub const KSBH_ABI_MINOR: u16 = 1;
pub const KSBH_ABI_PATCH: u16 = 0;

pub const KSBH_ABI_VERSION: KsbhAbiVersion = KsbhAbiVersion {
    major: KSBH_ABI_MAJOR,
    minor: KSBH_ABI_MINOR,
    patch: KSBH_ABI_PATCH,
    _padding: 0,
};

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct KsbhAbiVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,

    pub _padding: u16,
}

impl KsbhAbiVersion {
    /// Checks whether a module built against `self` can run on a host
    /// supporting `host`.
    ///
    /// Compatibility rule:
    ///
    /// - major must match exactly
    /// - module minor must be <= host minor
    /// - patch is ignored for compatibility
    pub const fn is_compatible_with_host(self, host: Self) -> bool {
        self.major == host.major && self.minor <= host.minor
    }
}
