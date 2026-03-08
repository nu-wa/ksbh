fn main() {
    println!("cargo:rustc-link-arg=-Wl,--export-dynamic-symbol=request_filter");
    println!("cargo:rustc-link-arg=-Wl,--export-dynamic-symbol=get_type");
}
