use crate::zengin::common::gothic2_dir;
use crate::zengin::script::main::FunctionId;
use crate::zengin::script::main::InstanceId;
use crate::zengin::script::main::ScriptManager;
use crate::zengin::script::main::SymbolId;
use crate::zengin::script::main::exports::mmcomando::zengine_scripts::manage::SymbolKind;
use daedalus_wasm::wasm::convert_daedalus_script;
use std::path::PathBuf;

/// Symbol printed in detail, including the script import calls it makes when
/// executed
const DEBUG_ID: u32 = 11145;

/// Script vm debug tool: prints small info for all symbols and detailed info
/// with the import calls for [`DEBUG_ID`]
#[test]
#[ignore]
fn debug_zengin_scripts() {
    let script_path = PathBuf::from(gothic2_dir() + "/_work/Data/Scripts/_compiled/GOTHIC.DAT");
    let wasm_bytes = convert_daedalus_script(&script_path);
    let mut executor = ScriptManager::new(&wasm_bytes);
    executor.load_symbol_table();

    executor.initialize_instances();
    let init_function_id = executor.get_function_id("init_newworld");
    executor.call_function(init_function_id);
    let startup_function_id = executor.get_function_id("startup_newworld");
    executor.call_function(startup_function_id);
    executor.call_npc_routines();
    executor.load_item_instances();
    executor.load_npc_instances();
    executor.load_dialogs_information();

    let symbols_num = executor.get_all_symbols_num();
    for index in 0..symbols_num {
        let Some(index) = SymbolId::new(index) else {
            continue;
        };
        let info = executor.get_symbol_info(index);
        let name = executor.call_get_name(index);
        match info.function {
            Some(function) => {
                println!(
                    "{}) kind({:?}), name({name}) fn({function})",
                    index.value(),
                    info.kind
                )
            }
            None => println!("{}) kind({:?}), name({name})", index.value(), info.kind),
        }
    }

    let debug_id = executor
        .symbol_table()
        .instance(DEBUG_ID as i32)
        .expect("DEBUG_ID should be a valid instance symbol");

    println!("\n=== detailed info for {DEBUG_ID} ===");
    let info = executor.get_symbol_info(debug_id.symbol());
    println!("info: {info:?}");
    println!("name: {:?}", executor.call_get_name(debug_id.symbol()));

    match info.kind {
        SymbolKind::Instance => debug_npc_routine(&mut executor, debug_id),
        _ => {
            println!("calling symbol as function");
            // DEBUG_ID is verified as an instance above, instances are callable
            executor.call_function(unsafe_wrap_instance_callable(debug_id));
        }
    }
}

/// Instances are callable in daedalus; the debug tool verified the kind above
fn unsafe_wrap_instance_callable(instance_id: InstanceId) -> FunctionId {
    FunctionId::from_symbol(instance_id.symbol())
}

/// Prints the routine entries of the npc instance and executes its active
/// routine state function, so its import calls show up in the recording
fn debug_npc_routine(executor: &mut ScriptManager, npc_id: InstanceId) {
    let hour = 8;
    let debug_info = {
        let data = executor.store.data();
        let Some(instance) = data.loaded_state.instance_data.get(&npc_id) else {
            println!("npc {npc_id} has NO loaded_state.instance_data entry");
            println!(
                "raw_instances contains {npc_id}: {}",
                data.raw_instances.contains_key(&npc_id)
            );
            return;
        };
        let routine_entries = format!("{:?}", instance.routine_enties);
        let active_entry = instance
            .get_routine_entry(hour * 60)
            .map(|entry| (entry.state_fn, format!("{entry:?}")));
        (routine_entries, active_entry)
    };
    println!("routine entries: {}", debug_info.0);

    let Some((state_fn, entry_debug)) = debug_info.1 else {
        println!("no routine entry active at {hour}:00");
        return;
    };
    println!("active entry at {hour}:00: {entry_debug}");
    let Some(state_fn) = state_fn else {
        println!("entry has no state function");
        return;
    };

    println!(
        "state_fn name: {:?}",
        executor.call_get_name(state_fn.symbol())
    );
    let anis = executor.call_routine_state_function(npc_id, state_fn);
    println!("played anis: {anis:?}");
}
