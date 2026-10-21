mod common;

use crate::common::WasmExecutor;
use daedalus_wasm::daedalus::{CallSignature, Function, Instruction as InstDD, Symbol, SymbolKind};

#[test]
fn test_call_npc_routine() {
    let symbols = &mut [
        Symbol::new(0, false, 1, None),
        Symbol::new(1, false, 1, None),
        Symbol::new(2, false, 200, None),
        Symbol::new(3, false, 200, None),
    ];

    let instructions = vec![InstDD::Return];
    let call_signature = CallSignature::new(0, false);
    symbols[1].set_function(Function::new("my-test", call_signature, &instructions));
    symbols[2].set_default_values(vec![1; 200]);
    symbols[3].set_default_values(vec![-1; 200]); // Invalid value for npc_routine index

    let mut executor = WasmExecutor::prepare_test(&[InstDD::PushInt(55), InstDD::Return], symbols);
    executor.call_npc_routine(2);
    executor.call_npc_routine(3);
    assert_eq!(executor.call_test_func(), 55);
}

#[test]
fn test_get_name() {
    let symbols = &mut [
        Symbol::new(0, false, 1, None),
        Symbol::new(1, false, 1, None),
    ];

    let mut executor = WasmExecutor::prepare_test(&[InstDD::PushInt(55), InstDD::Return], symbols);
    assert_eq!(executor.call_get_name(0), "0");
    assert_eq!(executor.call_get_name(1), "1");
    assert_eq!(executor.call_test_func(), 55);
}

#[test]
fn test_get_symbol_id() {
    let symbols = &mut [
        Symbol::new(0, false, 1, None),
        Symbol::new(1, false, 1, None),
        Symbol::new(2, false, 1, None),
    ];

    let mut executor = WasmExecutor::prepare_test(&[InstDD::PushInt(55), InstDD::Return], symbols);
    assert_eq!(executor.get_symbol_id("0"), 0);
    assert_eq!(executor.get_symbol_id("1"), 1);
    assert_eq!(executor.get_symbol_id("2"), 2);
    // Missing symbols return None instead of trapping, a trap would make the
    // component instance unusable for all further calls.
    assert_eq!(executor.try_get_symbol_id("3"), None);
}

#[test]
fn test_get_item_visual() {
    let symbols = &mut [
        Symbol::new(0, false, 1, None),
        Symbol::new(1, false, 1, None),
        Symbol::new(2, false, 200, None),
        Symbol::new(3, false, 200, None),
    ];
    symbols[2].set_default_values(vec![1; 200]);
    symbols[3].set_default_values(vec![0; 200]);

    let mut executor = WasmExecutor::prepare_test(&[InstDD::PushInt(55), InstDD::Return], symbols);
    assert_eq!(executor.get_item_visual(2).unwrap(), "1");
    assert!(executor.get_item_visual(3).is_none());

    assert_eq!(executor.get_item_visual_change(2).unwrap(), "1");
    assert!(executor.get_item_visual_change(3).is_none());
}

#[test]
fn test_get_npc_info() {
    // Class member data words: C_NPC hitpoints @ word 110 (offset 440),
    // hitpoints max @ word 111 (offset 444), name @ word 73 (offset 292)
    let hitpoints_word = 440 / 4;
    let hitpoints_max_word = 444 / 4;
    let name_word = 292 / 4;

    let mut values = vec![0; 200];
    values[hitpoints_word] = 40;
    values[hitpoints_max_word] = 400;
    // Daedalus string id of the npc name
    values[name_word] = 3;

    let symbols = &mut [
        Symbol::new(0, false, 1, None),
        Symbol::new(1, false, 1, None),
        Symbol::new(2, false, 200, None),
    ];
    symbols[2].set_default_values(values);

    let instructions = &[InstDD::PushInt(55), InstDD::Return];
    let mut executor = WasmExecutor::prepare_test(instructions, symbols);
    let info = executor.get_npc_info(2);
    assert_eq!(info.hitpoints, 40);
    assert_eq!(info.hitpoints_max, 400);
    assert_eq!(info.name, Some(3));

    // Empty instance data reads as zeroes
    symbols[2].set_default_values(vec![0; 200]);
    let mut executor = WasmExecutor::prepare_test(instructions, symbols);
    let info = executor.get_npc_info(2);
    assert_eq!(info.hitpoints, 0);
    assert_eq!(info.hitpoints_max, 0);
    assert_eq!(info.name, None);
}

#[test]
fn test_get_item_info() {
    // Class member data words: C_ITEM damagetotal @ word 90 (offset 360),
    // name @ word 73 (offset 292), value @ 88, weight @ 87, cond_atr @ 112,
    // cond_value @ 115, description @ 157, text @ 162, count @ 192
    let damage_total_word = 360 / 4;
    let name_word = 292 / 4;
    let weight_word = 348 / 4;
    let value_word = 352 / 4;
    let cond_atr_word = 448 / 4;
    let cond_value_word = 460 / 4;
    let description_word = 628 / 4;
    let text_word = 648 / 4;
    let count_word = 768 / 4;

    let mut values = vec![0; 200];
    values[damage_total_word] = 65;
    // Daedalus string id of the item name
    values[name_word] = 3;
    values[weight_word] = 10;
    values[value_word] = 400;
    values[cond_atr_word] = 4; // ATR_STRENGTH
    values[cond_value_word] = 40;
    values[description_word] = 4;
    values[text_word] = 5;
    values[count_word] = 2;

    let symbols = &mut [
        Symbol::new(0, false, 1, None),
        Symbol::new(1, false, 1, None),
        Symbol::new(2, false, 200, None),
    ];
    symbols[2].set_default_values(values);

    let instructions = &[InstDD::PushInt(55), InstDD::Return];
    let mut executor = WasmExecutor::prepare_test(instructions, symbols);
    let info = executor.get_item_info(2);
    assert_eq!(info.damage_total, 65);
    assert_eq!(info.name, Some(3));
    assert_eq!(info.value, 400);
    assert_eq!(info.weight, 10);
    assert_eq!(info.cond_attribute, vec![4, 0, 0]);
    assert_eq!(info.cond_value, vec![40, 0, 0]);
    assert_eq!(info.description, Some(4));
    assert_eq!(info.text, vec![5, 0, 0, 0, 0, 0]);
    assert_eq!(info.count, vec![2, 0, 0, 0, 0, 0]);

    symbols[2].set_default_values(vec![0; 200]);
    let mut executor = WasmExecutor::prepare_test(instructions, symbols);
    let info = executor.get_item_info(2);
    assert_eq!(info.damage_total, 0);
    assert_eq!(info.name, None);
    assert_eq!(info.value, 0);
    assert_eq!(info.weight, 0);
    assert_eq!(info.cond_attribute, vec![0, 0, 0]);
    assert_eq!(info.cond_value, vec![0, 0, 0]);
    assert_eq!(info.description, None);
    assert_eq!(info.text, vec![0, 0, 0, 0, 0, 0]);
    assert_eq!(info.count, vec![0, 0, 0, 0, 0, 0]);
}

#[test]
fn test_call_function() {
    let symbols = &mut [
        Symbol::new(0, false, 1, None),
        Symbol::new(1, false, 1, None),
        Symbol::new(2, false, 1, None),
    ];

    let instructions = vec![InstDD::Return];
    let call_signature = CallSignature::new(0, false);
    symbols[1].set_function(Function::new("my-test", call_signature, &instructions));

    let mut executor = WasmExecutor::prepare_test(&[InstDD::PushInt(55), InstDD::Return], symbols);
    executor.call_function(1);
}

#[test]
fn test_call_initialize_all_instances() {
    let symbols = &mut [
        Symbol::new(0, false, 1, None),
        Symbol::new(1, false, 1, None),
        Symbol::new(2, false, 200, None),
        Symbol::new(3, false, 200, None),
    ];

    let instructions = vec![InstDD::Return];
    let call_signature = CallSignature::new(0, false);
    symbols[1].set_kind(SymbolKind::Instance);
    symbols[1].set_function(Function::new("my-test", call_signature, &instructions));
    symbols[2].set_kind(SymbolKind::Instance);
    symbols[2].set_function(Function::new("my-test2", call_signature, &instructions));

    let mut executor = WasmExecutor::prepare_test(&[InstDD::PushInt(55), InstDD::Return], symbols);
    executor.initialize_all_instances();
}

#[test]
fn test_all_symbols_num() {
    let symbols = &mut [
        Symbol::new(0, false, 1, None),
        Symbol::new(1, false, 1, None),
        Symbol::new(2, false, 200, None),
        Symbol::new(3, false, 200, None),
    ];

    let mut executor = WasmExecutor::prepare_test(&[InstDD::PushInt(55), InstDD::Return], symbols);
    assert_eq!(executor.get_all_symbols_num(), 4);
}

#[test]
fn test_get_symbol_info() {
    use crate::common::exports::mmcomando::zengine_scripts::manage::SymbolKind;

    let class = 0_usize;
    let class_var = 1_usize;
    let func = 2_usize;
    let plain_var = 3_usize;

    let symbols = &mut [
        Symbol::new(class as u32, false, 1, None),
        Symbol::new(class_var as u32, true, 1, Some(0)),
        Symbol::new(func as u32, false, 1, None),
        Symbol::new(plain_var as u32, false, 1, None),
    ];
    symbols[class_var].set_parent(Some(class as u32));
    symbols[func].set_function(Function::new(
        "my-test",
        CallSignature::new(0, false),
        &[InstDD::Return],
    ));

    let mut executor = WasmExecutor::prepare_test(&[InstDD::PushInt(55), InstDD::Return], symbols);

    let info = executor.get_symbol_info(class as i32);
    assert_eq!(info.kind, SymbolKind::Int);
    assert_eq!(info.parent, None);
    assert_eq!(info.function, None);

    let info = executor.get_symbol_info(class_var as i32);
    assert_eq!(info.kind, SymbolKind::Int);
    assert_eq!(info.parent, Some(class as i32));
    assert_eq!(info.function, None);

    let info = executor.get_symbol_info(func as i32);
    assert_eq!(info.kind, SymbolKind::Int);
    assert_eq!(info.parent, None);
    assert!(info.function.is_some());

    let info = executor.get_symbol_info(plain_var as i32);
    assert_eq!(info.kind, SymbolKind::Int);
    assert_eq!(info.parent, None);
    assert_eq!(info.function, None);
}
