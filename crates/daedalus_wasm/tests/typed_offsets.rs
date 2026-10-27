//! Validates the hardcoded G2 class offsets against the installed scripts.

use std::path::PathBuf;

use daedalus_typed::{SymbolDecl, SymbolKind, TypedRegistry};
use daedalus_wasm::daedalus::{CompiledDaedalusScript, Symbol};

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
    let mut registry = TypedRegistry::new();
    for symbol in &data.symbols {
        registry.add_symbol(decl(symbol));
    }
    registry.finish();
    for (index, symbol) in data.symbols.iter().enumerate() {
        let index = index as u32;
        if registry.has_value_slot(index) {
            registry.write_values(index, symbol.default_values());
        }
    }
    Some(registry)
}

fn symbol_id(registry: &TypedRegistry, name: &str) -> u32 {
    (0..registry.symbol_count())
        .find(|index| {
            registry
                .name(*index as u32)
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
        })
        .expect("symbol exists") as u32
}

fn decl<'a>(symbol: &'a Symbol) -> SymbolDecl<'a> {
    SymbolDecl {
        name: symbol.name(),
        kind: convert_symbol_kind(symbol.kind()),
        is_const: symbol.is_const(),
        is_class_member: symbol.is_class_var(),
        address_or_offset: symbol.offset().unwrap_or(0),
        element_count: symbol.elements_count(),
        has_function: symbol.is_function_with_code(),
        parent: symbol.parent(),
    }
}

fn convert_symbol_kind(kind: daedalus_wasm::daedalus::SymbolKind) -> SymbolKind {
    use daedalus_wasm::daedalus::SymbolKind as Source;
    match kind {
        Source::Float => SymbolKind::Float,
        Source::Int => SymbolKind::Int,
        Source::String => SymbolKind::String,
        Source::Class => SymbolKind::Class,
        Source::Func => SymbolKind::Func,
        Source::Prototype => SymbolKind::Prototype,
        Source::Instance => SymbolKind::Instance,
        Source::VariableArgument => SymbolKind::VariableArgument,
    }
}

#[test]
fn gothic_dat_offsets_match() {
    let Some(mut registry) = build("GOTHIC.DAT") else {
        return;
    };
    assert!(registry.typed_count() > 0);
    assert!(registry.is_typed_class(daedalus_typed::ClassKind::Info));

    let self_var = symbol_id(&registry, "self");
    assert_eq!(registry.class_of(self_var), None);
    registry.write_symbol(self_var, 0, 123);
    assert_eq!(registry.read_symbol(self_var, 0), 123);

    let hero = symbol_id(&registry, "pc_hero");
    assert_eq!(
        registry.class_of(hero),
        Some(daedalus_typed::ClassKind::Npc)
    );
}

#[test]
fn menu_dat_offsets_match() {
    let Some(registry) = build("MENU.DAT") else {
        return;
    };
    assert!(registry.typed_count() > 0);
}
