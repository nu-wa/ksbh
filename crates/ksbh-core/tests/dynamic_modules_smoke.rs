use libloading::{Library, Symbol};
use std::env;

type FnGetDescriptor = unsafe extern "C" fn() -> ksbh_modules_abi::module_descriptor::ModuleDescriptor;

fn module_library_extension() -> &'static str {
    if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    }
}

fn resolve_module_dir() -> std::path::PathBuf {
    if let Ok(dir) = env::var("KSBH_MODULE_DIR") {
        return std::path::PathBuf::from(dir);
    }

    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("crates").join("target").join("debug"))
        .unwrap_or_else(|| manifest_dir.join("..").join("..").join("crates").join("target").join("debug"))
}

fn load_and_validate(lib_path: &std::path::Path) {
    // SAFETY: We trust the compiled cdylib modules
    let lib = unsafe { Library::new(lib_path) }
        .unwrap_or_else(|e| panic!("failed to load library {:?}: {e}", lib_path));

    // Get the module_descriptor symbol
    let descriptor_fn: Symbol<FnGetDescriptor> = unsafe { lib.get(b"module_descriptor") }
        .unwrap_or_else(|e| panic!("failed to get 'module_descriptor' symbol from {:?}: {e}", lib_path));

    let descriptor = unsafe { descriptor_fn() };

    // Validate magic
    assert_eq!(
        descriptor.magic,
        ksbh_modules_abi::version::KSBH_MAGIC,
        "magic mismatch for {:?}: expected 0x{:016X}, got 0x{:016X}",
        lib_path,
        ksbh_modules_abi::version::KSBH_MAGIC,
        descriptor.magic
    );

    // Validate ABI version compatibility
    assert!(
        descriptor.abi_version.is_compatible_with_host(ksbh_modules_abi::version::KSBH_ABI_VERSION),
        "ABI version incompatible for {:?}: module={:?}, host={:?}",
        lib_path,
        descriptor.abi_version,
        ksbh_modules_abi::version::KSBH_ABI_VERSION,
    );

    // Validate the module info is sane
    assert!(
        descriptor.info.registered_stages.len > 0,
        "module {:?} registered zero stages",
        lib_path
    );

    // handle_request_fn and free_module_response_fn must be non-null
    let handle_fn: *const () = descriptor.handle_request_fn as *const ();
    assert!(
        !handle_fn.is_null(),
        "handle_request_fn is null for {:?}",
        lib_path
    );

    let free_fn: *const () = descriptor.free_module_response_fn as *const ();
    assert!(
        !free_fn.is_null(),
        "free_module_response_fn is null for {:?}",
        lib_path
    );

    // Call handle_request with null context → should return null (no crash)
    let module_response = unsafe {
        (descriptor.handle_request_fn)(ksbh_modules_abi::types::KSBHModuleStage::Request, std::ptr::null())
    };

    // If a response was returned despite null ctx, free it
    if !module_response.is_null() {
        unsafe {
            // Pass null ctx - free_module_response ignores ctx anyway
            (descriptor.free_module_response_fn)(std::ptr::null(), module_response as *mut ksbh_modules_abi::types::ModuleResponse);
        }
    }

    // Drop the library
    drop(lib);
}

#[test]
fn load_all_modules_and_validate_descriptors() {
    let loops: usize = env::var("KSBH_DYNAMIC_SMOKE_LOOPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);

    let module_dir = resolve_module_dir();
    let ext = module_library_extension();

    let modules = [
        "http_to_https",
        "oidc",
        "proof_of_work",
        "rate_limit",
        "robots_txt",
    ];

    for mod_name in &modules {
        let lib_name = format!("lib{mod_name}.{ext}");
        let lib_path = module_dir.join(&lib_name);

        if !lib_path.exists() {
            // Try without 'lib' prefix
            let alt_lib_path = module_dir.join(format!("{mod_name}.{ext}"));
            if alt_lib_path.exists() {
                for i in 0..loops {
                    load_and_validate(&alt_lib_path);
                    if i % 100 == 0 && i > 0 {
                        eprintln!("  {mod_name}: {i}/{loops}");
                    }
                }
                continue;
            }
            panic!(
                "module artifact not found: tried {:?} and {:?}. Run `cargo build --workspace` first.",
                lib_path, alt_lib_path
            );
        }

        for i in 0..loops {
            load_and_validate(&lib_path);
            if i % 100 == 0 && i > 0 {
                eprintln!("  {mod_name}: {i}/{loops}");
            }
        }
    }

    eprintln!("Validated all 5 modules × {loops} iterations");
}

#[test]
fn single_module_oidc_validates() {
    let module_dir = resolve_module_dir();
    let ext = module_library_extension();
    let lib_path = module_dir.join(format!("liboidc.{ext}"));
    assert!(lib_path.exists(), "oidc module not found at {:?}", lib_path);
    load_and_validate(&lib_path);
}

#[test]
fn single_module_pow_validates() {
    let module_dir = resolve_module_dir();
    let ext = module_library_extension();
    let lib_path = module_dir.join(format!("libproof_of_work.{ext}"));
    assert!(lib_path.exists(), "pow module not found at {:?}", lib_path);
    load_and_validate(&lib_path);
}

#[test]
fn single_module_httptohttps_validates() {
    let module_dir = resolve_module_dir();
    let ext = module_library_extension();
    let lib_path = module_dir.join(format!("libhttp_to_https.{ext}"));
    assert!(lib_path.exists(), "http_to_https module not found at {:?}", lib_path);
    load_and_validate(&lib_path);
}

#[test]
fn single_module_ratelimit_validates() {
    let module_dir = resolve_module_dir();
    let ext = module_library_extension();
    let lib_path = module_dir.join(format!("librate_limit.{ext}"));
    assert!(lib_path.exists(), "rate_limit module not found at {:?}", lib_path);
    load_and_validate(&lib_path);
}

#[test]
fn single_module_robotstxt_validates() {
    let module_dir = resolve_module_dir();
    let ext = module_library_extension();
    let lib_path = module_dir.join(format!("librobots_txt.{ext}"));
    assert!(lib_path.exists(), "robots_txt module not found at {:?}", lib_path);
    load_and_validate(&lib_path);
}
