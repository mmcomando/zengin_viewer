//! Throwaway tool: converts the Gothic II menu script (MENU.DAT) to wasm and
//! dumps all symbols, to evaluate menu support in the viewer.
//! Run: cargo test -p daedalus_wasm --test test_menu_script -- --ignored --nocapture
mod common;

use crate::common::WasmExecutor;
use crate::common::gothic2_dir;
use daedalus_wasm::wasm::convert_daedalus_script;
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
    let data = daedalus_wasm::daedalus::CompiledDaedalusScript::from_file(&path).unwrap();

    println!("=== external functions (host imports) ===");
    let mut externals: HashMap<String, (u32, bool)> = HashMap::new();
    for external in daedalus_wasm::daedalus::external_functions(&data.symbols) {
        println!(
            "{} (params: {}, returns: {})",
            external.name, external.parameters_num, external.returns
        );
        externals
            .entry(external.name)
            .and_modify(|entry| {
                assert_eq!(entry.0, external.parameters_num);
                assert_eq!(entry.1, external.returns);
            })
            .or_insert((external.parameters_num, external.returns));
    }
    println!("\n=== unique externals: {} ===", externals.len());

    println!("\n=== convert to wasm ===");
    let wasm_bytes = convert_daedalus_script(&path);

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
