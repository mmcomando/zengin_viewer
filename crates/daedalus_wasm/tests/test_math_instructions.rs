mod common;

use crate::common::WasmExecutor;
use daedalus_wasm::daedalus::Instruction as InstDD;

#[test]
fn test_add() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(2),
        InstDD::PushInt(3),
        InstDD::Add,
        InstDD::Return,
    ]);
    assert_eq!(result, 5);
}

#[test]
fn test_sub() {
    // Daedalus pushes the right operand first: 5 - 2
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(2),
        InstDD::PushInt(5),
        InstDD::Sub,
        InstDD::Return,
    ]);
    assert_eq!(result, 3);
}

#[test]
fn test_mul() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(5),
        InstDD::PushInt(2),
        InstDD::Mul,
        InstDD::Return,
    ]);
    assert_eq!(result, 10);
}

#[test]
fn test_div() {
    // Daedalus pushes the right operand first: 20 / 10
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(10),
        InstDD::PushInt(20),
        InstDD::Div,
        InstDD::Return,
    ]);
    assert_eq!(result, 2);
}

#[test]
fn test_mod() {
    // Daedalus pushes the right operand first: 23 % 10
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(10),
        InstDD::PushInt(23),
        InstDD::Mod,
        InstDD::Return,
    ]);
    assert_eq!(result, 3);
}

#[test]
fn test_or() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(0b1010),
        InstDD::PushInt(0b0110),
        InstDD::OrBitwise,
        InstDD::Return,
    ]);
    assert_eq!(result, 0b1110);
}

#[test]
fn test_and() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(0b1010),
        InstDD::PushInt(0b0110),
        InstDD::AndBitwise,
        InstDD::Return,
    ]);
    assert_eq!(result, 0b0010);
}

#[test]
fn test_logical_or() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(1),
        InstDD::PushInt(0),
        InstDD::LogicalOr,
        InstDD::Return,
    ]);
    assert_eq!(result, 1);
}

#[test]
fn test_logical_and() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(1),
        InstDD::PushInt(0),
        InstDD::LogicalAnd,
        InstDD::Return,
    ]);
    assert_eq!(result, 0);
}

#[test]
fn test_eq() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(7),
        InstDD::PushInt(7),
        InstDD::Equal,
        InstDD::Return,
    ]);
    assert_eq!(result, 1);
}

#[test]
fn test_eq_negative() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(7),
        InstDD::PushInt(3),
        InstDD::Equal,
        InstDD::Return,
    ]);
    assert_eq!(result, 0);
}

#[test]
fn test_not_equal() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(7),
        InstDD::PushInt(3),
        InstDD::NotEqual,
        InstDD::Return,
    ]);
    assert_eq!(result, 1);
}

#[test]
fn test_not_equal_negative() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(7),
        InstDD::PushInt(7),
        InstDD::NotEqual,
        InstDD::Return,
    ]);
    assert_eq!(result, 0);
}

#[test]
fn test_lower() {
    // Daedalus pushes the right operand first: 2 < 3
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(3),
        InstDD::PushInt(2),
        InstDD::Lower,
        InstDD::Return,
    ]);
    assert_eq!(result, 1);
}

#[test]
fn test_lower_negative() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(3),
        InstDD::PushInt(3),
        InstDD::Lower,
        InstDD::Return,
    ]);
    assert_eq!(result, 0);
}

#[test]
fn test_lower_eq() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(3),
        InstDD::PushInt(3),
        InstDD::LowerEqual,
        InstDD::Return,
    ]);
    assert_eq!(result, 1);
}

#[test]
fn test_lower_eq_negative() {
    // 4 <= 3
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(3),
        InstDD::PushInt(4),
        InstDD::LowerEqual,
        InstDD::Return,
    ]);
    assert_eq!(result, 0);
}

#[test]
fn test_higher() {
    // Daedalus pushes the right operand first: 5 > 3
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(3),
        InstDD::PushInt(5),
        InstDD::Higher,
        InstDD::Return,
    ]);
    assert_eq!(result, 1);
}

#[test]
fn test_higher_negative() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(3),
        InstDD::PushInt(3),
        InstDD::Higher,
        InstDD::Return,
    ]);
    assert_eq!(result, 0);
}

#[test]
fn test_higher_eq() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(3),
        InstDD::PushInt(3),
        InstDD::HigherEqual,
        InstDD::Return,
    ]);
    assert_eq!(result, 1);
}

#[test]
fn test_higher_eq_negative() {
    // 2 >= 3
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(3),
        InstDD::PushInt(2),
        InstDD::HigherEqual,
        InstDD::Return,
    ]);
    assert_eq!(result, 0);
}

#[test]
fn test_un_plus() {
    let result =
        WasmExecutor::simple_return_test(&[InstDD::PushInt(42), InstDD::UnaryPlus, InstDD::Return]);
    assert_eq!(result, 42);
}

#[test]
fn test_un_minus() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(42),
        InstDD::UnaryMinus,
        InstDD::Return,
    ]);
    assert_eq!(result, -42);
}

#[test]
fn test_un_not() {
    let result =
        WasmExecutor::simple_return_test(&[InstDD::PushInt(0), InstDD::UnaryNot, InstDD::Return]);
    assert_eq!(result, 1);
}

#[test]
fn test_un_not_negative() {
    let result =
        WasmExecutor::simple_return_test(&[InstDD::PushInt(42), InstDD::UnaryNot, InstDD::Return]);
    assert_eq!(result, 0);
}

#[test]
fn test_complement() {
    let result =
        WasmExecutor::simple_return_test(&[InstDD::PushInt(0), InstDD::Complement, InstDD::Return]);
    assert_eq!(result, -1);
}

#[test]
fn test_complement_negative() {
    // ~42 = -43
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(42),
        InstDD::Complement,
        InstDD::Return,
    ]);
    assert_eq!(result, -43);
}

#[test]
fn test_complement_double_negation() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(42),
        InstDD::Complement,
        InstDD::Complement,
        InstDD::Return,
    ]);
    assert_eq!(result, 42);
}

#[test]
fn test_push_int() {
    let result = WasmExecutor::simple_return_test(&[InstDD::PushInt(42), InstDD::Return]);
    assert_eq!(result, 42);
}

#[test]
fn test_return() {
    let result = WasmExecutor::simple_return_test(&[
        InstDD::PushInt(42),
        InstDD::Return,
        InstDD::PushInt(0),
        InstDD::Return,
    ]);
    assert_eq!(result, 42);
}
