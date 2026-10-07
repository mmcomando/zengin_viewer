mod common;

use crate::common::WasmExecutor;
use daedalus_wasm::daedalus::Instruction as InstDD;

#[test]
fn test_if_early_return() {
    let instructions = [
        InstDD::PushInt(1),   // 0
        InstDD::JumpIf(16),   // 5
        InstDD::PushInt(100), // 10
        InstDD::Return,       // 15
        InstDD::PushInt(200), // 16
        InstDD::Return,       // 21
    ];
    let result = WasmExecutor::simple_return_test(&instructions);
    assert_eq!(result, 100);
}

#[test]
fn test_if_early_return_negative() {
    let instructions = [
        InstDD::PushInt(0),   // 0
        InstDD::JumpIf(16),   // 5
        InstDD::PushInt(100), // 10
        InstDD::Return,       // 15
        InstDD::PushInt(200), // 16
        InstDD::Return,       // 21
    ];
    let result = WasmExecutor::simple_return_test(&instructions);
    assert_eq!(result, 200);
}

#[test]
fn test_if_exit_jump() {
    let instructions = [
        InstDD::PushInt(1),   // 0
        InstDD::JumpIf(20),   // 5
        InstDD::PushInt(100), // 10
        InstDD::Jump(25),     // 15
        InstDD::PushInt(200), // 20
        InstDD::Return,       // 25
    ];
    let result = WasmExecutor::simple_return_test(&instructions);
    assert_eq!(result, 100);
}

#[test]
fn test_if_else_exit_jump() {
    let instructions = [
        InstDD::PushInt(1),   // 0
        InstDD::JumpIf(21),   // 5
        InstDD::PushInt(100), // 10
        InstDD::Return,       // 11
        InstDD::Jump(26),     // 16
        InstDD::PushInt(200), // 21
        InstDD::Return,       // 26
    ];
    let result = WasmExecutor::simple_return_test(&instructions);
    assert_eq!(result, 100);
}

#[test]
fn test_if_statement() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(1),
        InstDD::JumpIf(16),
        InstDD::PushInt(42),
        InstDD::Return,
        InstDD::PushInt(0),
        InstDD::Return,
    ]);
    assert_eq!(result, 42);
}

#[test]
fn test_if_statement_false_branch() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(0),
        InstDD::JumpIf(16),
        InstDD::PushInt(42),
        InstDD::Return,
        InstDD::PushInt(0),
        InstDD::Return,
    ]);
    assert_eq!(result, 0);
}

#[test]
fn test_if_else_statement() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(1),
        InstDD::JumpIf(21),
        InstDD::PushInt(42),
        InstDD::Return,
        InstDD::Jump(27),
        InstDD::PushInt(7),
        InstDD::Return,
        InstDD::PushInt(0),
        InstDD::Return,
    ]);
    assert_eq!(result, 42);
}

#[test]
fn test_if_else_statement_else_branch() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(0),
        InstDD::JumpIf(21),
        InstDD::PushInt(42),
        InstDD::Return,
        InstDD::Jump(27),
        InstDD::PushInt(7),
        InstDD::Return,
        InstDD::PushInt(0),
        InstDD::Return,
    ]);
    assert_eq!(result, 7);
}

#[test]
fn test_if_else_statement_continues_after_end() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(1),
        InstDD::JumpIf(26),
        InstDD::PushInt(42),
        InstDD::PushVar(0),
        InstDD::AssignInteger,
        InstDD::Jump(37),
        InstDD::PushInt(7),
        InstDD::PushVar(0),
        InstDD::AssignInteger,
        InstDD::PushVar(0),
        InstDD::Return,
    ]);
    assert_eq!(result, 42);
}

#[test]
fn test_if_else_statement_continues_after_end_else_branch() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(0),
        InstDD::JumpIf(26),
        InstDD::PushInt(42),
        InstDD::PushVar(0),
        InstDD::AssignInteger,
        InstDD::Jump(37),
        InstDD::PushInt(7),
        InstDD::PushVar(0),
        InstDD::AssignInteger,
        InstDD::PushVar(0),
        InstDD::Return,
    ]);
    assert_eq!(result, 7);
}
