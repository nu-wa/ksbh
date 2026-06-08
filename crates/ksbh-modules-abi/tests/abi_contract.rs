use ksbh_modules_abi::prelude::*;
use std::mem::size_of;

#[test]
fn type_layout_sanity() {
    // KSBHBytes: ptr(8) + len(8)
    assert_eq!(size_of::<KSBHBytes>(), 16);
    // KSBHString: inner KSBHBytes
    assert_eq!(size_of::<KSBHString>(), 16);
    // KSBHKVString: key(16) + value(16)
    assert_eq!(size_of::<KSBHKVString>(), 32);
    // KSBHKVBytes: key(16) + value(16)
    assert_eq!(size_of::<KSBHKVBytes>(), 32);
    // KSBHSliceBytes: ptr(8) + len(8)
    assert_eq!(size_of::<KSBHSliceBytes>(), 16);
    // KSBHSliceStrings: ptr(8) + len(8)
    assert_eq!(size_of::<KSBHSliceStrings>(), 16);
    // KSBHSliceKVStrings: ptr(8) + len(8)
    assert_eq!(size_of::<KSBHSliceKVStrings>(), 16);
    // SessionID: [u8; 16]
    assert_eq!(size_of::<SessionID>(), 16);
    // KSBHModuleKind: repr(u16) = 2 bytes
    assert_eq!(size_of::<KSBHModuleKind>(), 2);
    // KSBHModuleStage: repr(u16) = 2 bytes
    assert_eq!(size_of::<KSBHModuleStage>(), 2);
    // KSBHModuleStages: ptr(8) + len(8)
    assert_eq!(size_of::<KSBHModuleStages>(), 16);
    // KSBHHostCtxHandle: u64
    assert_eq!(size_of::<KSBHHostCtxHandle>(), 8);
    // KSBHHostFnReturn: repr(u16) = 2 bytes
    assert_eq!(size_of::<KSBHHostFnReturn>(), 2);
    // ModuleResponse: body(16) + decision(2) + padding(6) + headers(16) + status_code(2) + padding(6) = 48
    assert_eq!(size_of::<ModuleResponse>(), 48);
    // KSBHModuleDecision: repr(u16) = 2 bytes
    assert_eq!(size_of::<KSBHModuleDecision>(), 2);
    // LogLevel: repr(u16) = 2 bytes
    assert_eq!(size_of::<LogLevel>(), 2);
    // KsbhAbiVersion: major(2) + minor(2) + patch(2) + padding(2)
    assert_eq!(size_of::<KsbhAbiVersion>(), 8);
    // ModuleInfo: kind(2) + padding(6) + name(16) + registered_stages(16) = 40
    assert_eq!(size_of::<ModuleInfo>(), 40);
    // ModuleDescriptor: magic(8) + version(8) + info(40) + 2 fn ptrs(8 each) = 72
    assert_eq!(size_of::<ModuleDescriptor>(), 72);
}

#[test]
fn version_compatibility_matrix() {
    let v0_1_0 = KsbhAbiVersion {
        major: 0,
        minor: 1,
        patch: 0,
        _padding: 0,
    };
    let v0_0_0 = KsbhAbiVersion {
        major: 0,
        minor: 0,
        patch: 0,
        _padding: 0,
    };
    let v0_2_0 = KsbhAbiVersion {
        major: 0,
        minor: 2,
        patch: 0,
        _padding: 0,
    };
    let v1_0_0 = KsbhAbiVersion {
        major: 1,
        minor: 0,
        patch: 0,
        _padding: 0,
    };

    // Same version → compatible
    assert!(v0_1_0.is_compatible_with_host(v0_1_0));
    // Module minor < host minor → compatible
    assert!(v0_0_0.is_compatible_with_host(v0_1_0));
    // Module minor > host minor → incompatible
    assert!(!v0_2_0.is_compatible_with_host(v0_1_0));
    // Major mismatch → incompatible
    assert!(!v1_0_0.is_compatible_with_host(v0_1_0));
    // Module major > host major → incompatible
    assert!(!v1_0_0.is_compatible_with_host(v0_0_0));
}

#[test]
fn magic_constant() {
    assert_eq!(KSBH_MAGIC, 0x4B_53_42_48_5F_41_42_49);
    assert_ne!(KSBH_MAGIC, 0);
}

#[test]
fn bytes_roundtrip() {
    // Empty
    let data: &[u8] = &[];
    let abi = KSBHBytes::from_slice(data);
    assert_eq!(abi.as_slice(), data);

    // Non-empty
    let data = b"hello world";
    let abi = KSBHBytes::from_slice(data);
    assert_eq!(abi.as_slice(), data);

    // With null bytes
    let data = b"before\0after";
    let abi = KSBHBytes::from_slice(data);
    assert_eq!(abi.as_slice(), data);

    // Single byte
    let data = &[42u8];
    let abi = KSBHBytes::from_slice(data);
    assert_eq!(abi.as_slice(), data);
}

#[test]
fn bytes_null_pointer_returns_empty() {
    let abi = KSBHBytes {
        ptr: std::ptr::null(),
        len: 0,
    };
    assert_eq!(abi.as_slice(), &[]);

    // Non-zero len with null ptr also returns empty
    let abi = KSBHBytes {
        ptr: std::ptr::null(),
        len: 42,
    };
    assert_eq!(abi.as_slice(), &[]);
}

#[test]
fn string_roundtrip() {
    // Empty
    let abi = KSBHString::from_str("");
    assert_eq!(abi.as_str(), "");

    // ASCII
    let abi = KSBHString::from_str("hello");
    assert_eq!(abi.as_str(), "hello");

    // Unicode
    let abi = KSBHString::from_str("héllo 世界");
    assert_eq!(abi.as_str(), "héllo 世界");
}

#[test]
fn string_invalid_utf8_returns_empty() {
    let invalid = [0xFF, 0xFE, 0xFD];
    let abi = KSBHString {
        inner: KSBHBytes::from_slice(&invalid),
    };
    assert_eq!(abi.as_str(), "");

    // Partial valid then invalid
    let mixed = [b'h', b'e', 0xFF, b'l', b'o'];
    let abi = KSBHString {
        inner: KSBHBytes::from_slice(&mixed),
    };
    assert_eq!(abi.as_str(), "");
}

#[test]
fn build_kv_entries() {
    use ksbh_modules_abi::convert::build_kv_entries;

    // Empty iterator
    let empty: Vec<(&str, &str)> = vec![];
    let result = build_kv_entries(empty);
    assert!(result.is_empty());

    // Single pair
    let result = build_kv_entries([("key1", "value1")]);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].key.as_str(), "key1");
    assert_eq!(result[0].value.as_str(), "value1");

    // Multiple pairs
    let result = build_kv_entries([("a", "1"), ("b", "2"), ("c", "3")]);
    assert_eq!(result.len(), 3);
    assert_eq!(result[1].key.as_str(), "b");
    assert_eq!(result[1].value.as_str(), "2");
}

#[test]
fn abi_version_struct_equality() {
    let a = KsbhAbiVersion {
        major: 0,
        minor: 1,
        patch: 0,
        _padding: 0,
    };
    let b = KSBH_ABI_VERSION;
    assert_eq!(a, b);
    assert_eq!(b.major, 0);
    assert_eq!(b.minor, 1);
    assert_eq!(b.patch, 0);
}

#[test]
fn module_decision_variants() {
    assert_eq!(KSBHModuleDecision::Pass as u16, 0);
    assert_eq!(KSBHModuleDecision::Stop as u16, 1);
    assert_eq!(KSBHModuleDecision::Error as u16, 2);
}

#[test]
fn host_fn_return_variants() {
    assert_eq!(KSBHHostFnReturn::Success as u16, 0);
    assert_eq!(KSBHHostFnReturn::HostFailure as u16, 1);
    assert_eq!(KSBHHostFnReturn::BadArgument as u16, 2);
    assert_eq!(KSBHHostFnReturn::NotFound as u16, 3);
}
