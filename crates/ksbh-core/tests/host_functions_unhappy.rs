fn ensure_store_initialized() -> ::std::sync::Arc<
    ksbh_core::storage::redis_hashmap::RedisHashMap<
        ksbh_core::storage::module_session_key::ModuleSessionKey,
        ::std::vec::Vec<u8>,
    >,
> {
    let store = ::std::sync::Arc::new(ksbh_core::storage::redis_hashmap::RedisHashMap::new(
        Some(tokio::time::Duration::from_secs(60)),
        Some(tokio::time::Duration::from_secs(60)),
        Some(::std::sync::Arc::new(ksbh_core::storage::Storage::empty())),
    ));
    ksbh_core::modules::abi::host_functions::set_module_sessions(store.clone());
    ksbh_core::modules::abi::host_functions::set_module_metrics(store.clone());
    store
}

fn session_uuid() -> [u8; 16] {
    *uuid::Uuid::new_v4().as_bytes()
}

fn module_name_bytes() -> &'static [u8] {
    b"host-functions-unhappy"
}

fn data_key_bytes() -> &'static [u8] {
    b"payload"
}

#[test]
fn session_get_returns_false_when_session_id_ptr_is_null() {
    let _store = ensure_store_initialized();
    let mut out_ptr = ::std::ptr::dangling::<u8>();
    let mut out_len: usize = 123;
    let module_name = module_name_bytes();
    let data_key = data_key_bytes();

    let ok = unsafe {
        ksbh_core::modules::abi::host_functions::host_session_get(
            ::std::ptr::null(),
            module_name.as_ptr(),
            module_name.len(),
            data_key.as_ptr(),
            data_key.len(),
            &mut out_ptr,
            &mut out_len,
        )
    };

    assert!(!ok);
    assert_eq!(out_ptr as usize, 0x1);
    assert_eq!(out_len, 123);
}

#[test]
fn session_get_returns_false_when_out_ptr_or_out_len_is_null() {
    let _store = ensure_store_initialized();
    let session_id = session_uuid();
    let module_name = module_name_bytes();
    let data_key = data_key_bytes();
    let mut out_ptr = ::std::ptr::null();
    let mut out_len: usize = 0;

    let null_out_ptr = unsafe {
        ksbh_core::modules::abi::host_functions::host_session_get(
            session_id.as_ptr(),
            module_name.as_ptr(),
            module_name.len(),
            data_key.as_ptr(),
            data_key.len(),
            ::std::ptr::null_mut(),
            &mut out_len,
        )
    };
    let null_out_len = unsafe {
        ksbh_core::modules::abi::host_functions::host_session_get(
            session_id.as_ptr(),
            module_name.as_ptr(),
            module_name.len(),
            data_key.as_ptr(),
            data_key.len(),
            &mut out_ptr,
            ::std::ptr::null_mut(),
        )
    };

    assert!(!null_out_ptr);
    assert!(!null_out_len);
}

#[test]
fn session_get_rejects_module_name_len_above_max() {
    let _store = ensure_store_initialized();
    let session_id = session_uuid();
    let module_name = [b'a'; 257];
    let data_key = data_key_bytes();
    let mut out_ptr = ::std::ptr::null();
    let mut out_len: usize = 0;

    let ok = unsafe {
        ksbh_core::modules::abi::host_functions::host_session_get(
            session_id.as_ptr(),
            module_name.as_ptr(),
            module_name.len(),
            data_key.as_ptr(),
            data_key.len(),
            &mut out_ptr,
            &mut out_len,
        )
    };
    assert!(!ok);
}

#[test]
fn session_set_rejects_data_len_above_max() {
    let _store = ensure_store_initialized();
    let session_id = session_uuid();
    let module_name = module_name_bytes();
    let data_key = data_key_bytes();
    let data = [b'x'; 16];

    let ok = unsafe {
        ksbh_core::modules::abi::host_functions::host_session_set(
            session_id.as_ptr(),
            module_name.as_ptr(),
            module_name.len(),
            data_key.as_ptr(),
            data_key.len(),
            data.as_ptr(),
            1024 * 1024 + 1,
        )
    };
    assert!(!ok);
}

#[test]
fn session_set_rejects_invalid_utf8_module_name_or_data_key() {
    let _store = ensure_store_initialized();
    let session_id = session_uuid();
    let invalid_module = [0xff, 0xfe];
    let invalid_key = [0xff, 0xfe];
    let data = [b'x'; 4];

    let bad_module = unsafe {
        ksbh_core::modules::abi::host_functions::host_session_set(
            session_id.as_ptr(),
            invalid_module.as_ptr(),
            invalid_module.len(),
            b"key".as_ptr(),
            b"key".len(),
            data.as_ptr(),
            data.len(),
        )
    };
    let bad_key = unsafe {
        ksbh_core::modules::abi::host_functions::host_session_set(
            session_id.as_ptr(),
            b"module".as_ptr(),
            b"module".len(),
            invalid_key.as_ptr(),
            invalid_key.len(),
            data.as_ptr(),
            data.len(),
        )
    };

    assert!(!bad_module);
    assert!(!bad_key);
}

#[test]
fn session_set_with_ttl_rejects_oversized_payload_and_invalid_utf8() {
    let _store = ensure_store_initialized();
    let session_id = session_uuid();
    let data = [b'x'; 8];
    let invalid = [0xff, 0xfe];

    let oversized = unsafe {
        ksbh_core::modules::abi::host_functions::host_session_set_with_ttl(
            session_id.as_ptr(),
            b"module".as_ptr(),
            b"module".len(),
            b"key".as_ptr(),
            b"key".len(),
            data.as_ptr(),
            1024 * 1024 + 1,
            60,
        )
    };
    let invalid_module = unsafe {
        ksbh_core::modules::abi::host_functions::host_session_set_with_ttl(
            session_id.as_ptr(),
            invalid.as_ptr(),
            invalid.len(),
            b"key".as_ptr(),
            b"key".len(),
            data.as_ptr(),
            data.len(),
            60,
        )
    };
    let invalid_key = unsafe {
        ksbh_core::modules::abi::host_functions::host_session_set_with_ttl(
            session_id.as_ptr(),
            b"module".as_ptr(),
            b"module".len(),
            invalid.as_ptr(),
            invalid.len(),
            data.as_ptr(),
            data.len(),
            60,
        )
    };

    assert!(!oversized);
    assert!(!invalid_module);
    assert!(!invalid_key);
}

#[test]
fn session_free_is_noop_for_null_ptr_or_zero_len() {
    let byte = [0u8; 1];
    unsafe {
        ksbh_core::modules::abi::host_functions::host_session_free(
            b"module".as_ptr(),
            b"module".len(),
            ::std::ptr::null(),
            1,
        );
        ksbh_core::modules::abi::host_functions::host_session_free(
            b"module".as_ptr(),
            b"module".len(),
            byte.as_ptr(),
            0,
        );
    }
}

#[test]
fn session_get_then_free_then_get_again_returns_valid_new_allocation() {
    let _store = ensure_store_initialized();
    let session_id = session_uuid();
    let module_name = module_name_bytes();
    let data_key = data_key_bytes();
    let payload = b"hello-host-functions";

    let set_ok = unsafe {
        ksbh_core::modules::abi::host_functions::host_session_set(
            session_id.as_ptr(),
            module_name.as_ptr(),
            module_name.len(),
            data_key.as_ptr(),
            data_key.len(),
            payload.as_ptr(),
            payload.len(),
        )
    };
    assert!(set_ok);

    let mut out_ptr = ::std::ptr::null();
    let mut out_len: usize = 0;
    let first_get = unsafe {
        ksbh_core::modules::abi::host_functions::host_session_get(
            session_id.as_ptr(),
            module_name.as_ptr(),
            module_name.len(),
            data_key.as_ptr(),
            data_key.len(),
            &mut out_ptr,
            &mut out_len,
        )
    };
    assert!(first_get);
    assert_eq!(out_len, payload.len());
    let first_bytes = unsafe { ::std::slice::from_raw_parts(out_ptr, out_len) };
    assert_eq!(first_bytes, payload);
    unsafe {
        ksbh_core::modules::abi::host_functions::host_session_free(
            module_name.as_ptr(),
            module_name.len(),
            out_ptr,
            out_len,
        );
    }

    let mut second_out_ptr = ::std::ptr::null();
    let mut second_out_len: usize = 0;
    let second_get = unsafe {
        ksbh_core::modules::abi::host_functions::host_session_get(
            session_id.as_ptr(),
            module_name.as_ptr(),
            module_name.len(),
            data_key.as_ptr(),
            data_key.len(),
            &mut second_out_ptr,
            &mut second_out_len,
        )
    };
    assert!(second_get);
    let second_bytes = unsafe { ::std::slice::from_raw_parts(second_out_ptr, second_out_len) };
    assert_eq!(second_bytes, payload);
    unsafe {
        ksbh_core::modules::abi::host_functions::host_session_free(
            module_name.as_ptr(),
            module_name.len(),
            second_out_ptr,
            second_out_len,
        );
    }
}

#[test]
fn metrics_good_boy_rejects_null_or_non_16_byte_key() {
    let _store = ensure_store_initialized();
    let key = [1u8; 16];
    let null_key = unsafe {
        ksbh_core::modules::abi::host_functions::host_metrics_good_boy(::std::ptr::null(), 16)
    };
    let short_key =
        unsafe { ksbh_core::modules::abi::host_functions::host_metrics_good_boy(key.as_ptr(), 15) };
    let long_key =
        unsafe { ksbh_core::modules::abi::host_functions::host_metrics_good_boy(key.as_ptr(), 17) };

    assert!(!null_key);
    assert!(!short_key);
    assert!(!long_key);
}

#[test]
fn metrics_get_score_returns_zero_for_null_non16_or_missing_value() {
    let _store = ensure_store_initialized();
    let key = [1u8; 16];
    let null_key = unsafe {
        ksbh_core::modules::abi::host_functions::host_metrics_get_score(::std::ptr::null(), 16)
    };
    let short_key = unsafe {
        ksbh_core::modules::abi::host_functions::host_metrics_get_score(key.as_ptr(), 15)
    };
    let missing = unsafe {
        ksbh_core::modules::abi::host_functions::host_metrics_get_score(key.as_ptr(), 16)
    };

    assert_eq!(null_key, 0);
    assert_eq!(short_key, 0);
    assert_eq!(missing, 0);
}

#[test]
fn session_set_with_ttl_accepts_u64_max_ttl_and_value_is_immediately_readable() {
    let _store = ensure_store_initialized();
    let session_id = session_uuid();
    let module_name = module_name_bytes();
    let data_key = data_key_bytes();
    let payload = b"ttl-max";

    let set_ok = unsafe {
        ksbh_core::modules::abi::host_functions::host_session_set_with_ttl(
            session_id.as_ptr(),
            module_name.as_ptr(),
            module_name.len(),
            data_key.as_ptr(),
            data_key.len(),
            payload.as_ptr(),
            payload.len(),
            u64::MAX,
        )
    };
    assert!(set_ok);

    let mut out_ptr = ::std::ptr::null();
    let mut out_len: usize = 0;
    let get_ok = unsafe {
        ksbh_core::modules::abi::host_functions::host_session_get(
            session_id.as_ptr(),
            module_name.as_ptr(),
            module_name.len(),
            data_key.as_ptr(),
            data_key.len(),
            &mut out_ptr,
            &mut out_len,
        )
    };
    assert!(get_ok);
    assert_eq!(out_len, payload.len());
    let read = unsafe { ::std::slice::from_raw_parts(out_ptr, out_len) };
    assert_eq!(read, payload);
    unsafe {
        ksbh_core::modules::abi::host_functions::host_session_free(
            module_name.as_ptr(),
            module_name.len(),
            out_ptr,
            out_len,
        );
    }
}
