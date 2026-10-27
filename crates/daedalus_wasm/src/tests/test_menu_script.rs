//! Throwaway tool: converts the Gothic II menu script (MENU.DAT) to wasm and
//! dumps all symbols, to evaluate menu support in the viewer.
//! Run: cargo test -p daedalus_wasm --lib tests::test_menu_script -- --ignored --nocapture

use crate::daedalus::SymbolKind;
use crate::tests::common::WasmExecutor;
use crate::tests::common::gothic2_dir;
use crate::wasm::convert_daedalus_script;
use std::collections::HashMap;
use std::path::PathBuf;
use wasmparser::Parser;

fn menu_main_script_path() -> PathBuf {
    gothic2_dir().join("_work/Data/Scripts/_compiled/MENU.DAT")
}

#[test]
#[ignore]
fn menu_convert_and_dump_symbols() {
    let path = menu_main_script_path();
    let data = crate::daedalus::CompiledDaedalusScript::from_file(&path).unwrap();

    println!("=== external functions (host imports) ===");
    let mut externals: HashMap<String, (u32, bool)> = HashMap::new();
    for symbol in &data.symbols {
        let Some(func) = &symbol.function else {
            continue;
        };
        if !func.is_external {
            continue;
        }
        let parameters_num = func.call_signature.parameters_num;
        let returns = func.call_signature.returns;
        println!(
            "{} (params: {}, returns: {})",
            func.name, parameters_num, returns
        );
        externals
            .entry(func.name.clone())
            .and_modify(|entry| {
                assert_eq!(entry.0, parameters_num);
                assert_eq!(entry.1, returns);
            })
            .or_insert((parameters_num, returns));
    }
    println!("\n=== unique externals: {} ===", externals.len());

    println!("\n=== convert to wasm ===");
    let wasm_bytes = convert_daedalus_script(&path);

    println!("\n=== class layout ===");
    for class in ["c_menu", "c_menu_item"] {
        let Some(class_index) = data
            .symbols
            .iter()
            .position(|symbol| symbol.kind == SymbolKind::Class && symbol.name == class)
        else {
            println!("{class}: missing");
            continue;
        };
        let class_index = class_index as u32;
        let prefix = format!("{class}.");
        let mut members: Vec<_> = data
            .symbols
            .iter()
            .filter_map(|symbol| {
                if symbol.parent != Some(class_index) {
                    return None;
                }
                Some((
                    symbol
                        .name
                        .strip_prefix(&prefix)
                        .unwrap_or(symbol.name.as_str()),
                    symbol.offset?,
                    symbol.elements_count,
                ))
            })
            .collect();
        members.sort_by_key(|(_, offset, _)| *offset);
        println!("{class}:");
        for (name, offset, elements_count) in members {
            println!("  {name} offset={offset} elements={elements_count}");
        }
    }

    println!("=== component imports (host functions the menu script calls) ===");
    for payload in Parser::new(0).parse_all(&wasm_bytes) {
        if let Ok(wasmparser::Payload::ComponentImportSection(section)) = payload {
            for import in section {
                let import = import.unwrap();
                println!("import {}", import.name.0);
            }
        }
    }

    println!("\n=== symbols ===");

    let mut executor = WasmExecutor::new(&wasm_bytes);

    let symbols_num = executor.get_all_symbols_num();
    println!("symbols: {symbols_num}");
    for index in 0..symbols_num {
        let info = executor.get_symbol_info(index);
        let name = executor.call_get_name(index as u32);
        match info.function {
            Some(function) => println!(
                "{index}) kind({:?}), name({name}) fn({function})",
                info.kind
            ),
            None => println!("{index}) kind({:?}), name({name})", info.kind),
        }
    }

    println!("\n=== probes ===");
    for probe in ["c_menu", "c_menu_item", "menu_main", "menu_options"] {
        println!("probe {probe}: {:?}", executor.try_get_symbol_id(probe));
    }
}
