fn main() {
    // Re-run the CMake build only when the vendored C++ sources change. Without
    // any rerun-if-changed, Cargo re-runs this script whenever *any* file in the
    // package changes (e.g. src/bindings.rs), re-triggering the whole C++ build.
    // build.rs itself is always tracked by Cargo.
    println!("cargo:rerun-if-changed=vendor/ZenKitCAPI");

    let mut cfg = cmake::Config::new("vendor/ZenKitCAPI");

    // CMAKE_DEBUG_POSTFIX to empty otherwire shared library name might change
    // and dependant project might not link in debug build
    cfg.define("CMAKE_DEBUG_POSTFIX", "");

    // Needed otherwise there is an error:
    // file INSTALL cannot find
    // "... ZenKit-sys/vendor/ZenKitCAPI/vendor/ZenKit/include/phoenix"
    cfg.define("ZK_ENABLE_INSTALL", "NO");

    if std::env::var("CARGO_FEATURE_STATIC").is_ok() {
        cfg.define("BUILD_SHARED_LIBS", "OFF");
    }

    let dst = cfg.build();

    println!("cargo:rustc-link-search=native={}/lib/", dst.display());
    println!("cargo:rustc-link-lib=dylib=zenkitcapi");
}
