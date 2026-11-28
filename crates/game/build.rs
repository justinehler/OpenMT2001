fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let target = std::env::var("TARGET").unwrap();
    if target.ends_with("windows-msvc") && std::env::var_os("CARGO_FEATURE_NATIVE").is_some() {
        // DX12 pipeline creation and the retained intro handoff exceed the
        // Windows 1 MiB main-thread reserve in a debug executable.
        println!("cargo:rustc-link-arg-bin=game=/STACK:16777216");
    }
}
