mod common;

use crate::common::WasmExecutor;
use crate::common::gothic2_main_script_path;
use daedalus_wasm::daedalus::CompiledDaedalusScript;
use daedalus_wasm::daedalus::Instruction as InstDD;
use daedalus_wasm::wasm::convert_daedalus_script;

use std::sync::OnceLock;

// Cached script data for improved tests performance
fn get_main_script_data() -> &'static CompiledDaedalusScript {
    static DATA: OnceLock<CompiledDaedalusScript> = OnceLock::new();
    DATA.get_or_init(|| {
        let path = gothic2_main_script_path();
        CompiledDaedalusScript::from_file(&path).unwrap()
    })
}

#[test]
fn real_parse() {
    get_main_script_data();
}

#[test]
fn real_data_content() {
    let data = get_main_script_data();
    assert_eq!(data.symbols.len(), 74651);
}

#[test]
fn test_real_ta_smalltalk() {
    let data = get_main_script_data();

    let func_address = data.get_function_address("ta_smalltalk").unwrap();
    let instructions = &[
        InstDD::PushInt(1),
        InstDD::PushInt(2),
        InstDD::PushInt(3),
        InstDD::PushInt(4),
        InstDD::PushInt(5),
        InstDD::Call(func_address),
        InstDD::PushInt(1),
        InstDD::Return,
    ];

    let mut executor = WasmExecutor::real_func_test(data, &["ta_smalltalk"], instructions);
    executor.call_test_func();
    assert_eq!(executor.call_count("ta_min"), 1);
}

#[test]
fn real_printdebugs() {
    let data = get_main_script_data();
    let func_address = data.get_function_address("printdebugs").unwrap();
    let instructions = &[
        InstDD::PushInt(1),
        InstDD::Call(func_address),
        InstDD::PushInt(1),
        InstDD::Return,
    ];
    let mut executor = WasmExecutor::real_func_test(data, &["printdebugs"], instructions);

    executor.call_test_func();
    assert_eq!(executor.call_count("printdebug"), 2);
}

#[test]
fn real_printdebugnpc() {
    let data = get_main_script_data();
    let func_address = data.get_function_address("printdebugnpc").unwrap();
    let instructions = &[
        InstDD::PushInt(1),
        InstDD::PushInt(2),
        InstDD::Call(func_address),
        InstDD::PushInt(1),
        InstDD::Return,
    ];
    let mut executor = WasmExecutor::real_func_test(data, &["printdebugnpc"], instructions);

    executor.call_test_func();
    assert_eq!(executor.call_count("inttostring"), 1);
    assert_eq!(executor.call_count("concatstrings"), 6);
    assert_eq!(executor.call_count("printdebuginstch"), 1);
}

#[test]
fn real_printglobals() {
    let data = get_main_script_data();
    let func_address = data.get_function_address("printglobals").unwrap();
    let instructions = &[
        InstDD::PushInt(1),
        InstDD::Call(func_address),
        InstDD::PushInt(1),
        InstDD::Return,
    ];
    let mut executor =
        WasmExecutor::real_func_test(data, &["printdebugnpc", "printglobals"], instructions);

    executor.call_test_func();
    assert_eq!(executor.call_count("hlp_isvalidnpc"), 4);
    assert_eq!(executor.call_count("printdebuginstch"), 1);
    assert_eq!(executor.call_count("inttostring"), 1);
    assert_eq!(executor.call_count("hlp_isvaliditem"), 1);
    assert_eq!(executor.call_count("concatstrings"), 6);
}

#[test]
fn real_b_awake_stoneguardian() {
    let data = get_main_script_data();
    let func_address = data.get_function_address("b_awake_stoneguardian").unwrap();
    let instructions = &[
        InstDD::PushInt(1),
        InstDD::Call(func_address),
        InstDD::PushInt(1),
        InstDD::Return,
    ];
    let mut executor = WasmExecutor::real_func_test(data, &["b_awake_stoneguardian"], instructions);

    executor.call_test_func();
    assert_eq!(executor.call_count("wld_playeffect"), 0);
}

#[test]
fn test_real_b_setnpcvisual() {
    let data = get_main_script_data();
    let func_address = data.get_function_address("b_setnpcvisual").unwrap();
    let instructions = &[
        InstDD::PushInstance(1156),
        InstDD::PushInt(2),
        InstDD::PushInt(3),
        InstDD::PushInt(4),
        InstDD::PushInt(5),
        InstDD::PushInt(6),
        InstDD::Call(func_address),
        InstDD::PushInt(1),
        InstDD::Return,
    ];
    let mut executor = WasmExecutor::real_func_test(data, &["b_setnpcvisual"], instructions);

    executor.call_test_func();
    assert_eq!(executor.call_count("mdl_setvisualbody"), 1);
    assert_eq!(executor.call_count("mdl_setvisual"), 1);
    assert_eq!(executor.call_count("mdl_setmodelscale"), 0);
}

#[test]
fn test_none_100_xardas() {
    let data = get_main_script_data();
    let func_address = data.get_function_address("none_100_xardas").unwrap();
    let instructions = &[
        InstDD::SetInstance(1156),
        InstDD::Call(func_address),
        InstDD::PushInt(1),
        InstDD::Return,
    ];
    let mut executor = WasmExecutor::real_func_test(
        data,
        &[
            // Needed functions
            "b_addfightskill",
            "b_raiserealfighttalentpercent",
            "b_raisefighttalent",
            "b_setfightskills",
            "b_givenpctalents",
            "b_setnpcvisual",
            "b_setattributestochapter",
            "npc_default",
            // Main instantiate function we want to call
            "none_100_xardas",
        ],
        instructions,
    );

    executor.call_test_func();
    assert_eq!(executor.call_count("mdl_setmodelscale"), 1);
    assert_eq!(executor.call_count("mdl_setmodelfatness"), 1);
    assert_eq!(executor.call_count("mdl_applyoverlaymds"), 1);
    assert_eq!(executor.call_count("mdl_setvisualbody"), 1);
    assert_eq!(executor.call_count("mdl_setvisual"), 1);
    assert_eq!(executor.call_count("npc_settalentskill"), 18);
}

#[test]
fn real_c_isquarternearest() {
    let data = get_main_script_data();
    let func_address = data.get_function_address("c_isquarternearest").unwrap();
    let instructions = &[
        InstDD::PushInstance(1156),
        InstDD::PushInt(1),
        InstDD::Call(func_address),
        InstDD::Return,
    ];
    let mut executor = WasmExecutor::real_func_test(
        data,
        &[
            "printplus",
            "c_getdisttocenter",
            // Main instantiate function we want to call
            "c_isquarternearest",
        ],
        instructions,
    );

    executor.call_test_func();
    assert_eq!(executor.call_count("npc_getdisttowp"), 25);
}

#[test]
fn real_b_makerangerreadytoleavemeeting() {
    let data = get_main_script_data();
    let func_address = data
        .get_function_address("b_makerangerreadytoleavemeeting")
        .unwrap();
    let instructions = &[
        InstDD::PushInstance(1156),
        InstDD::PushInt(1),
        InstDD::Call(func_address),
        InstDD::Return,
    ];
    let mut executor =
        WasmExecutor::real_func_test(data, &["b_makerangerreadytoleavemeeting"], instructions);

    executor.call_test_func();
    assert_eq!(executor.call_count("npc_hasitems"), 2);
    assert_eq!(executor.call_count("ai_unequiparmor"), 1);
    assert_eq!(executor.call_count("ai_equipbestarmor"), 1);
    assert_eq!(executor.call_count("npc_removeinvitems"), 2);
}

#[test]
// This function ignores returned values from called functions
fn real_b_extro_avi() {
    let data = get_main_script_data();
    let func_address = data.get_function_address("b_extro_avi").unwrap();
    let instructions = &[
        InstDD::PushInstance(1156),
        InstDD::Call(func_address),
        InstDD::PushInt(1),
        InstDD::Return,
    ];
    let mut executor = WasmExecutor::real_func_test(data, &["b_extro_avi"], instructions);

    executor.call_test_func();
    assert_eq!(executor.call_count("npc_isdead"), 2);
    assert_eq!(executor.call_count("playvideo"), 4);
    assert_eq!(executor.call_count("exitsession"), 1);
}

#[test]
fn real_c_npcisinquarter() {
    let data = get_main_script_data();
    let func_address = data.get_function_address("c_npcisinquarter").unwrap();
    let instructions = &[
        InstDD::PushInstance(1156),
        InstDD::Call(func_address),
        InstDD::Return,
    ];
    let mut executor = WasmExecutor::real_func_test(
        data,
        &[
            "printplus",
            "c_getdisttocenter",
            "c_isquarternearest",
            "c_npcisinquarter",
        ],
        instructions,
    );

    executor.call_test_func();
    assert_eq!(executor.call_count("npc_getdisttowp"), 48);
}

#[test]
fn real_c_diegotoofar() {
    let data = get_main_script_data();
    let func_address = data.get_function_address("c_diegotoofar").unwrap();
    let instructions = &[
        InstDD::PushInt(100),
        InstDD::Call(func_address),
        InstDD::Return,
    ];
    let mut executor = WasmExecutor::real_func_test(data, &["c_diegotoofar"], instructions);

    executor.call_test_func();
    assert_eq!(executor.call_count("npc_getdisttowp"), 17);
    assert_eq!(executor.call_count("npc_getheighttonpc"), 1);
    assert_eq!(executor.call_count("wld_getplayerportalguild"), 1);
}

#[test]
// One branch of if/else ignores value
fn real_b_openshell() {
    let data = get_main_script_data();
    let func_address = data.get_function_address("b_openshell").unwrap();
    let instructions = &[
        InstDD::PushInt(100),
        InstDD::Call(func_address),
        InstDD::PushInt(1),
        InstDD::Return,
    ];
    let mut executor = WasmExecutor::real_func_test(
        data,
        &["b_playerfinditem", "b_say_overlay", "b_openshell"],
        instructions,
    );

    executor.call_test_func();
    assert_eq!(executor.call_count("hlp_random"), 1);
    assert_eq!(executor.call_count("npc_isplayer"), 1);
    assert_eq!(executor.call_count("npc_getdisttowp"), 0);
}

#[test]
// Last statement is if/else where both branches return
fn real_b_mm_wispdetect() {
    let data = get_main_script_data();
    let func_address = data.get_function_address("b_mm_wispdetect").unwrap();
    let instructions = &[
        InstDD::PushInstance(1156),
        InstDD::Call(func_address),
        InstDD::Return,
    ];
    let mut executor = WasmExecutor::real_func_test(
        data,
        &[
            "b_getwispdetecteditem",
            "b_wispdetecteditem",
            "b_startotherroutine",
            "b_removenpc",
            "b_mm_wispdetect",
        ],
        instructions,
    );

    executor.call_test_func();
    assert_eq!(executor.call_count("npc_clearaiqueue"), 1);
    assert_eq!(executor.call_count("ai_gotonpc"), 1);
    assert_eq!(executor.call_count("npc_perceiveall"), 1);
    assert_eq!(executor.call_count("npc_getdisttonpc"), 2);
    assert_eq!(executor.call_count("wld_detectnpc"), 1);
    assert_eq!(executor.call_count("hlp_getinstanceid"), 2);
}

#[test]
// Has empty else statement
fn real_zs_talk_end() {
    let data = get_main_script_data();
    let func_address = data.get_function_address("zs_talk_end").unwrap();
    let instructions = &[
        InstDD::Call(func_address),
        InstDD::PushInt(1),
        InstDD::Return,
    ];
    let mut executor = WasmExecutor::real_func_test(
        data,
        &["c_npcisbotheredbyplayerroomguild", "zs_talk_end"],
        instructions,
    );

    executor.call_test_func();
    assert_eq!(executor.call_count("wld_getguildattitude"), 1);
    assert_eq!(executor.call_count("npc_getattitude"), 1);
    assert_eq!(executor.call_count("wld_getplayerportalguild"), 2);
    assert_eq!(executor.call_count("npc_setrefusetalk"), 1);
}

#[test]
fn real_find_init_newworld_symbol() {
    let data = get_main_script_data();

    let mut executor = WasmExecutor::real_func_test(
        data,
        &["ignore_all_symbols"],
        &[InstDD::PushInt(1), InstDD::Return],
    );

    assert_eq!(executor.get_symbol_id("init_newworld"), 28949);
}

#[test]
fn real_all_functions() {
    let path = gothic2_main_script_path();
    let _ = convert_daedalus_script(&path);
}

#[test]
fn real_test_xardas_npc_routine() {
    let data = get_main_script_data();

    let func_address = data.get_function_address("none_100_xardas").unwrap();
    let instructions = &[
        InstDD::SetInstance(11156),
        InstDD::Call(func_address),
        InstDD::PushInt(1),
        InstDD::Return,
    ];

    let mut executor = WasmExecutor::real_func_test(data, &[], instructions);

    println!("-- call_test_func --");
    executor.call_test_func();
    println!("-- call_npc_routine --");
    executor.call_npc_routine(11156);
}

#[test]
fn real_test_dialog_info() {
    let data = get_main_script_data();

    let instructions = &[InstDD::PushInt(1), InstDD::Return];

    let mut executor = WasmExecutor::real_func_test(data, &["dia_lares_hallo"], instructions);

    // Initialize memory of given instance
    executor.set_instance(26934);
    executor.call_function(26934);
    // Get dialog information from this instance
    let info = executor.get_dialog_info(26934);
    assert_eq!(info.npc, 12081);
    assert_eq!(info.order, 2);
    assert_eq!(info.condition, 26935);
    assert_eq!(info.information, 26936);
    assert!(!info.permanent);
    assert!(info.important);
}

/// Routine state functions request their animations via `AI_PlayAni(BS)`. The
/// setup of `ZS_STAND_WP` requests none, its `_loop` function transitions into
/// the guard pose on the first invocation.
#[test]
fn real_routine_state_played_animations() {
    let data = get_main_script_data();

    let mut executor =
        WasmExecutor::real_func_test(data, &[], &[InstDD::PushInt(1), InstDD::Return]);

    // Like in the viewer, instance memories have to be initialized before npc
    // instances are used
    executor.initialize_all_instances();
    let npc_instance = executor.get_symbol_id("none_100_xardas");
    executor.set_instance(npc_instance as u32);

    let state_fn = executor.get_symbol_id("zs_stand_wp");
    executor.call_function(state_fn as u32);
    assert!(executor.play_ani_calls().is_empty());

    // Loop functions return a value and have to be called through the bool path.
    // The loop returns LOOP_CONTINUE which is 0, so only the side effects matter.
    let loop_fn = executor.get_symbol_id("zs_stand_wp_loop");
    let _ = executor.call_function_bool(loop_fn);

    let &(npc, ani_id, body_state) = executor.play_ani_calls().first().unwrap();
    // The npc argument must be the instance the state was executed for
    assert_eq!(npc, npc_instance);
    assert_eq!(body_state, 0);
    assert_eq!(executor.call_get_name(ani_id as u32), "T_STAND_2_LGUARD");
}

/// The symbol lookup returns `None` for missing symbols, a trap would make
/// the component instance unusable for all further calls.
#[test]
fn real_get_symbol_id_missing_symbol() {
    let data = get_main_script_data();

    let mut executor = WasmExecutor::real_func_test(
        data,
        &["zs_stand_wp_loop"],
        &[InstDD::PushInt(1), InstDD::Return],
    );

    assert_eq!(executor.try_get_symbol_id("definitely_not_a_symbol"), None);
    let expected = executor.get_symbol_id("zs_stand_wp_loop");
    assert_eq!(
        executor.try_get_symbol_id("zs_stand_wp_loop"),
        Some(expected)
    );
    // The instance stays usable after the failed lookup
    assert_eq!(executor.call_get_name(expected as u32), "zs_stand_wp_loop");
}
