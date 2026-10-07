mod common;

use crate::common::WasmExecutor;
use daedalus_wasm::daedalus::Instruction as InstDD;
use daedalus_wasm::daedalus::Symbol;

#[test]
fn test_assign_integer_global_memory() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(1337),
        InstDD::PushVar(0),
        InstDD::AssignInteger,
        InstDD::PushVar(0),
        InstDD::Return,
    ]);
    assert_eq!(result, 1337);
}

#[test]
fn test_add_to_global_memory() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(10),
        InstDD::PushVar(0),
        InstDD::AssignInteger,
        InstDD::PushInt(5),
        InstDD::PushVar(0),
        InstDD::AddTo,
        InstDD::PushVar(0),
        InstDD::Return,
    ]);
    assert_eq!(result, 15);
}

#[test]
fn test_sub_to_global_memory() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(10),
        InstDD::PushVar(0),
        InstDD::AssignInteger,
        InstDD::PushInt(3),
        InstDD::PushVar(0),
        InstDD::SubTo,
        InstDD::PushVar(0),
        InstDD::Return,
    ]);
    assert_eq!(result, 7);
}

#[test]
fn test_push_array_var_global_memory() {
    let symbols = &[Symbol::new(0, false, 2, None)];
    let result = WasmExecutor::return_test(
        &[
            InstDD::PushInt(10),
            InstDD::PushArrayVar(0, 0),
            InstDD::AssignInteger,
            InstDD::PushInt(20),
            InstDD::PushArrayVar(0, 1),
            InstDD::AssignInteger,
            InstDD::PushArrayVar(0, 0),
            InstDD::PushArrayVar(0, 1),
            InstDD::Add,
            InstDD::Return,
        ],
        symbols,
    );
    assert_eq!(result, 30);
}

#[test]
fn test_push_array_var_class_member_memory() {
    let class_a = 0;
    let class_b = 1;
    let array_var = 2;

    let symbols = &[
        Symbol::new(0, false, 2, None),
        Symbol::new(1, false, 2, None),
        Symbol::new(2, true, 2, Some(0)),
    ];
    let result = WasmExecutor::return_test(
        &[
            InstDD::SetInstance(class_a),
            InstDD::PushInt(10),
            InstDD::PushArrayVar(array_var, 0),
            InstDD::AssignInteger,
            InstDD::PushInt(20),
            InstDD::PushArrayVar(array_var, 1),
            InstDD::AssignInteger,
            InstDD::SetInstance(class_b),
            InstDD::PushInt(30),
            InstDD::PushArrayVar(array_var, 0),
            InstDD::AssignInteger,
            InstDD::PushInt(40),
            InstDD::PushArrayVar(array_var, 1),
            InstDD::AssignInteger,
            InstDD::SetInstance(class_a),
            InstDD::PushArrayVar(array_var, 0),
            InstDD::PushArrayVar(array_var, 1),
            InstDD::SetInstance(class_b),
            InstDD::PushArrayVar(array_var, 0),
            InstDD::PushArrayVar(array_var, 1),
            InstDD::Add,
            InstDD::Add,
            InstDD::Add,
            InstDD::Return,
        ],
        symbols,
    );
    assert_eq!(result, 100);
}

#[test]
fn test_assign_integer_class_member_memory() {
    let class_a = 0;
    let class_b = 1;
    let var_a = 2;
    let var_b = 3;

    let symbols = &[
        Symbol::new(0, false, 2, None),
        Symbol::new(1, false, 2, None),
        Symbol::new(2, true, 2, Some(0)),
        Symbol::new(3, true, 2, Some(4)),
    ];
    let result = WasmExecutor::return_test(
        &[
            // Assign to class 0
            InstDD::SetInstance(class_a),
            InstDD::PushInt(10),
            InstDD::PushVar(var_a),
            InstDD::AssignInteger,
            InstDD::PushInt(200),
            InstDD::PushVar(var_b),
            InstDD::AssignInteger,
            // Assign to class 1
            InstDD::SetInstance(class_b),
            InstDD::PushInt(3000),
            InstDD::PushVar(var_a),
            InstDD::AssignInteger,
            InstDD::PushInt(40000),
            InstDD::PushVar(var_b),
            InstDD::AssignInteger,
            // Get value from class 0
            InstDD::SetInstance(class_a),
            InstDD::PushVar(var_a),
            InstDD::PushVar(var_b),
            // Get value from class 0
            InstDD::SetInstance(class_b),
            InstDD::PushVar(var_a),
            InstDD::PushVar(var_b),
            // Add all
            InstDD::Add,
            InstDD::Add,
            InstDD::Add,
            InstDD::Return,
        ],
        symbols,
    );
    assert_eq!(result, 40000 + 3000 + 200 + 10);
}
