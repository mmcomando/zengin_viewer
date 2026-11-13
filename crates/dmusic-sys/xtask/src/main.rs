use std::path::PathBuf;

fn main() {
    let header = "../wrapper.h";

    let bindings = bindgen::Builder::default()
        .header(header)
        .allowlist_item("Dm.*")
        .default_enum_style(bindgen::EnumVariation::ModuleConsts)
        .clang_arg("-I../vendor/dmusic/include")
        .clang_arg("-DDM_STATIC=1")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from("../src/bindings.rs");
    bindings
        .write_to_file(&out_path)
        .expect("Couldn't write bindings!");

    // Mirror ZenKit-sys: strip enum name prefixes so the generated constants
    // read as `DmResult::SUCCESS` instead of `DmResult::DmResult_SUCCESS`.
    let mut text =
        std::fs::read_to_string(&out_path).expect("Should be able to read generated bindings");
    for (from, to) in [
        ("DmResult_", ""),
        ("DmLogLevel_", ""),
        ("DmTiming_", ""),
        ("DmEmbellishment_", ""),
        ("DmLoader_DOWNLOAD", "DOWNLOAD"),
        ("DmLoader_DEFAULT", "DEFAULT"),
        ("DmRender_SHORT", "SHORT"),
        ("DmRender_FLOAT", "FLOAT"),
        ("DmRender_STEREO", "STEREO"),
    ] {
        text = text.replace(from, to);
    }
    std::fs::write(&out_path, text).expect("Couldn't write bindings with additional changes");

    println!("Bindings successfully generated at src/bindings.rs");
}
