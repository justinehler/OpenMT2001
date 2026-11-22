fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let target = std::env::var("TARGET").unwrap();
    if target.ends_with("windows-msvc") && std::env::var_os("CARGO_FEATURE_NATIVE").is_some() {
        // DX12 pipeline creation and the retained intro handoff exceed the
        // Windows 1 MiB main-thread reserve in a debug executable.
        println!("cargo:rustc-link-arg-bin=game=/STACK:16777216");
    }
    if std::env::var_os("CARGO_FEATURE_LUA4").is_none() {
        return;
    }
    if target == "wasm32-unknown-unknown" {
        panic!(
            "lua4 needs a C runtime with setjmp/longjmp; use a supported native target or a separately configured browser runtime"
        );
    }
    println!("cargo:rerun-if-changed=third_party/lua4");
    println!("cargo:rerun-if-changed=src/lua4_shim.c");
    let mut build = cc::Build::new();
    build
        .include("third_party/lua4/include")
        .include("third_party/lua4/src")
        .define("_CRT_SECURE_NO_WARNINGS", None)
        .warnings(false);
    for source in [
        "lapi", "lcode", "ldebug", "ldo", "lfunc", "lgc", "llex", "lmem", "lobject", "lparser",
        "lstate", "lstring", "ltable", "ltm", "lundump", "lvm", "lzio",
    ] {
        build.file(format!("third_party/lua4/src/{source}.c"));
    }
    build.file("src/lua4_shim.c").compile("openmt_lua4");
    if target.contains("linux") {
        println!("cargo:rustc-link-lib=m");
    }
}
