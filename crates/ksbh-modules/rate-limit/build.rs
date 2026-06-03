fn main() {
    if cfg!(all(target_os = "linux", target_env = "gnu")) {
        println!("cargo:rustc-link-arg=-Wl,--export-dynamic-symbol=request_filter");
        println!("cargo:rustc-link-arg=-Wl,--export-dynamic-symbol=get_type");
    }
}
