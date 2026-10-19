//! Helper test: prints daedalus class member offsets, used to verify the
//! member offsets hardcoded in the manage exports (`base_module.wat`).
//! Requires `GOTHIC2_DIR` pointing at the Gothic II install directory.

use daedalus_wasm::daedalus::CompiledDaedalusScript;
use std::path::PathBuf;

#[test]
#[ignore]
fn print_class_member_offsets() {
    let path = PathBuf::from(
        std::env::var("GOTHIC2_DIR").expect("GOTHIC2_DIR must point at the Gothic II install dir")
            + "/_work/Data/Scripts/_compiled/GOTHIC.DAT",
    );
    let script = CompiledDaedalusScript::from_file(&path).expect("GOTHIC.DAT readable");

    let classes: Vec<String> = script
        .symbols
        .iter()
        .filter(|symbol| symbol.kind() == daedalus_wasm::daedalus::SymbolKind::Class)
        .map(|symbol| symbol.name().to_uppercase())
        .collect();
    println!("classes: {classes:?}");
    for class in classes {
        let Some(class_index) = script
            .symbols
            .iter()
            .position(|symbol| symbol.name().to_uppercase() == class)
        else {
            continue;
        };
        let class_index = class_index as u32;
        println!("=== {class} ===");
        for symbol in &script.symbols {
            if symbol.parent() == Some(class_index) && symbol.offset().is_some() {
                println!(
                    "{:>8} {:>18} count({:>3})",
                    symbol.offset().unwrap_or_default(),
                    symbol.name(),
                    symbol.elements_count()
                );
            }
        }
    }
}
