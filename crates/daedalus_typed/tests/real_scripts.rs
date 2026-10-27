//! Validates the hardcoded G2 class offsets against the installed scripts.

use std::path::PathBuf;

use daedalus_typed::TypedRegistry;
use daedalus_wasm::daedalus::CompiledDaedalusScript;
use daedalus_wasm::symbol_store::SymbolStore;

fn gothic2_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("GOTHIC2_DIR") {
        return Some(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME")?;
    let path = PathBuf::from(home).join(".local/share/Steam/steamapps/common/Gothic II");
    path.is_dir().then_some(path)
}

fn build(file: &str) -> Option<TypedRegistry> {
    let path = gothic2_dir()?
        .join("_work/Data/Scripts/_compiled")
        .join(file);
    if !path.is_file() {
        return None;
    }
    let data = CompiledDaedalusScript::from_file(&path).unwrap();
    let mut store = SymbolStore::from_symbols(&data.symbols);
    Some(TypedRegistry::build(&mut store))
}

#[test]
fn gothic_dat_offsets_match() {
    let Some(registry) = build("GOTHIC.DAT") else {
        return;
    };
    assert!(
        registry.issues().is_empty(),
        "GOTHIC.DAT layout mismatch: {:?}",
        registry.issues()
    );
    assert!(registry.typed_count() > 0);
    assert!(registry.is_typed_class(daedalus_typed::ClassKind::Info));
}

#[test]
fn menu_dat_offsets_match() {
    let Some(registry) = build("MENU.DAT") else {
        return;
    };
    assert!(
        registry.issues().is_empty(),
        "MENU.DAT layout mismatch: {:?}",
        registry.issues()
    );
    assert!(registry.typed_count() > 0);
}
