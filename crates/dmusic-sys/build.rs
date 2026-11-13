use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// `GothicKit/dmusic` checkout, normally the `vendor/dmusic` submodule.
/// `DMUSIC_DIR` overrides it for local development.
fn source_dir(manifest_dir: &Path) -> PathBuf {
    if let Some(dir) = env::var_os("DMUSIC_DIR") {
        return PathBuf::from(dir);
    }
    manifest_dir.join("vendor/dmusic")
}

fn collect_c(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.display()));
    for entry in entries {
        let path = entry.expect("readable directory entry").path();
        if path.is_dir() {
            collect_c(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "c") {
            files.push(path);
        }
    }
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let source = source_dir(&manifest_dir);
    let header = source.join("include/dmusic.h");
    if !header.is_file() {
        panic!(
            "dmusic sources not found at {}\n\
             add the submodule with:\n  \
             git submodule add https://github.com/GothicKit/dmusic.git crates/dmusic-sys/vendor/dmusic\n  \
             git submodule update --init --recursive\n\
             or set DMUSIC_DIR to a checkout",
            source.display()
        );
    }

    println!("cargo:rerun-if-env-changed=DMUSIC_DIR");
    println!("cargo:rerun-if-changed={}", source.display());

    let include = source.join("include");
    let src = source.join("src");
    let tsf = source.join("vendor/TinySoundFont");
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    let mut files = Vec::new();
    collect_c(&src, &mut files);
    files.retain(|path| {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        match target_os.as_str() {
            // glibc has C11 <threads.h>, so the portable shim is not needed.
            "linux" | "freebsd" | "netbsd" | "openbsd" | "dragonfly" => {
                name != "Thread.Posix.c" && name != "Thread.Win32.c"
            }
            "windows" => name != "Thread.Posix.c",
            _ => name != "Thread.Win32.c",
        }
    });
    files.push(tsf.join("tsf.c"));

    let mut build = cc::Build::new();
    build
        .files(&files)
        .include(&include)
        .include(&src)
        .include(&tsf)
        .define("DM_STATIC", "1")
        .flag_if_supported("-std=c11")
        .flag_if_supported("-fPIC")
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-function");
    match target_os.as_str() {
        "linux" | "freebsd" | "netbsd" | "openbsd" | "dragonfly" => {
            build.define("_DM_USE_NATIVE_THREAD", "1");
        }
        "windows" => {}
        _ => {
            // Expose the POSIX pthread mutex helpers used by the shim.
            build.define("_POSIX_C_SOURCE", "200809L");
            build.define("_XOPEN_SOURCE", "700");
        }
    }
    if cfg!(unix) {
        build.flag("-pthread");
    }
    build.compile("dmusic");

    if cfg!(unix) {
        println!("cargo:rustc-link-lib=pthread");
    }
}
