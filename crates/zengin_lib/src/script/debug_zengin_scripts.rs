use crate::common::gothic_dat_path;
use crate::script::bindings::exports::mmcomando::zengine_scripts::manage::SymbolKind;
use crate::script::manager::ScriptManager;
use crate::script::state::ScriptMessages;
use crate::script::symbol_id::FunctionId;
use crate::script::symbol_id::InstanceId;
use crate::script::symbol_id::SymbolId;
use daedalus_wasm::wasm::convert_daedalus_script;

/// Symbol printed in detail, including the script import calls it makes when
/// executed
const DEBUG_ID: u32 = 11145;

/// Script vm debug tool: prints small info for all symbols and detailed info
/// with the import calls for [`DEBUG_ID`]
#[test]
#[ignore = "manual script VM debug tool"]
fn debug_zengin_scripts() {
    let script_path = gothic_dat_path();
    let wasm_bytes = convert_daedalus_script(&script_path);
    let mut executor = ScriptManager::new(&wasm_bytes, ScriptMessages::default());
    executor.load_symbol_table();

    executor.initialize_instances();
    let init_function_id = executor
        .try_get_function_id("init_newworld")
        .expect("init_newworld function");
    executor.call_function(init_function_id);
    let startup_function_id = executor
        .try_get_function_id("startup_newworld")
        .expect("startup_newworld function");
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
                );
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

    if info.kind == SymbolKind::Instance {
        debug_npc_routine(&mut executor, debug_id);
    } else {
        println!("calling symbol as function");
        // DEBUG_ID is verified as an instance above, instances are callable
        executor.call_function(unsafe_wrap_instance_callable(debug_id));
    }
}

/// Instances are callable in daedalus; the debug tool verified the kind above
fn unsafe_wrap_instance_callable(instance_id: InstanceId) -> FunctionId {
    FunctionId::from_symbol(instance_id.symbol())
}

/// Verifies `Npc_ExchangeRoutine` name resolution: the suffix is the `C_Npc.id`
/// member, so `none_100_xardas` (instance symbol id 11156) must resolve
/// `rtn_start_100`, not `rtn_start_11156`.
#[test]
#[ignore = "manual script VM debug tool"]
fn debug_exchange_routine() {
    let script_path = gothic_dat_path();
    let wasm_bytes = convert_daedalus_script(&script_path);
    let mut executor = ScriptManager::new(&wasm_bytes, ScriptMessages::default());
    executor.load_symbol_table();
    executor.initialize_instances();

    let symbol = executor.get_symbol_id("none_100_xardas");
    let Some(npc) = executor.symbol_table().instance(symbol.value()) else {
        println!("none_100_xardas is not an instance symbol");
        return;
    };
    println!("none_100_xardas instance id: {}", npc.value());

    executor.exchange_routine(npc, "START");

    let entries = executor
        .npc_instances()
        .get(&npc)
        .map_or_default(|instance| {
            instance
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .routine
                .entries
                .len()
        });
    println!("routine entries after exchange: {entries}");
    assert_ne!(entries, 0, "rtn_start_100 did not register routine entries");
}

/// Verifies the save/restore of duplicated-spawn copies against the real script.
#[test]
#[ignore = "manual script VM debug tool"]
fn debug_npc_copy_save_restore() {
    let script_path = gothic_dat_path();
    let wasm_bytes = convert_daedalus_script(&script_path);
    let mut executor = ScriptManager::new(&wasm_bytes, ScriptMessages::default());
    executor.load_symbol_table();
    executor.initialize_instances();

    let source = executor
        .symbol_instance("ywolf")
        .expect("ywolf should be an instance symbol");
    let first = executor.store.data_mut().allocate_npc_object(source);
    assert_eq!(first, source, "the first spawn keeps the symbol id");
    let copy = executor.store.data_mut().allocate_npc_object(source);
    assert_ne!(copy, source, "the second spawn is a separate object");
    assert!(
        executor.symbol_table().instance(copy.value()).is_some(),
        "copy id resolves as an instance"
    );

    let snapshot = executor.snapshot_save();
    assert!(
        snapshot
            .instances
            .iter()
            .any(|values| values.id == copy.value()),
        "copy typed words are saved in the instances section"
    );

    let mut restored = ScriptManager::new(&wasm_bytes, ScriptMessages::default());
    restored.load_symbol_table();
    restored.initialize_instances();
    restored.restore_npc_copies(&snapshot.instances);

    let copy_id = u32::try_from(copy.value()).expect("copy id");
    assert!(restored.store.data().typed.is_copy(copy_id));
    assert_eq!(
        restored.store.data().typed.copy_source(copy_id),
        Some(u32::try_from(source.value()).expect("source id"))
    );
    assert!(restored.store.data().typed.npc(copy_id).is_some());
    println!(
        "npc copy({}) restored from source({})",
        copy.value(),
        source.value()
    );
}

/// Prints the hitpoints of a npc instance, verifying the `get-npc-info` export
#[test]
#[ignore = "manual script VM debug tool"]
fn debug_npc_hitpoints() {
    let script_path = gothic_dat_path();
    let wasm_bytes = convert_daedalus_script(&script_path);
    let mut executor = ScriptManager::new(&wasm_bytes, ScriptMessages::default());
    executor.load_symbol_table();
    executor.initialize_instances();

    for name in ["pc_hero", "vlk_449_lares"] {
        let symbol = executor.get_symbol_id(name);
        let Some(npc) = executor.symbol_table().instance(symbol.value()) else {
            println!("symbol({name}) is not an instance");
            continue;
        };
        println!("{name} hitpoints: {:?}", executor.hitpoints(npc));
    }
}

/// Prints the routine entries of the npc instance and executes its active
/// routine state function, so its import calls show up in the recording
fn debug_npc_routine(executor: &mut ScriptManager, npc_id: InstanceId) {
    let hour = 8;
    let debug_info = {
        let Some(instance) = executor.npc_instances().get(&npc_id) else {
            println!("npc {npc_id} has NO loaded_state.npc_instances entry");
            return;
        };
        let instance = instance
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let routine_entries = format!("{:?}", instance.routine.entries);
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
    let functions = executor.resolve_routine_functions(state_fn);
    executor.call_routine_start(npc_id, functions);
    let ended = executor.call_routine_loop(npc_id, functions);
    println!(
        "animation changes: {:?}, loop ended: {ended}",
        executor.messages().animation.take()
    );
}

/// Documents why the player uses the `pc_hero` instance: the `hero` instance
/// global has no data of its own (hitpoints 0), the player data lives in
/// `PC_HERO`
#[test]
#[ignore = "manual script VM debug tool"]
fn debug_player_instance_hitpoints() {
    let script_path = gothic_dat_path();
    let wasm_bytes = convert_daedalus_script(&script_path);
    let mut executor = ScriptManager::new(&wasm_bytes, ScriptMessages::default());
    executor.load_symbol_table();
    executor.initialize_instances();

    let Some(raw) = SymbolId::new(1620) else {
        println!("1620 is not a valid symbol id");
        return;
    };
    println!("symbol(1620) name: {:?}", executor.call_get_name(raw));
    let Some(npc) = executor.symbol_table().instance(1620) else {
        println!("symbol(1620) is not an instance");
        return;
    };
    println!("player instance hitpoints: {:?}", executor.hitpoints(npc));
}
