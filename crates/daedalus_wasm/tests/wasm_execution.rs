mod common;

use crate::common::WasmExecutor;
use daedalus_wasm::daedalus::Instruction as InstDD;

#[test]
fn test_add_daedalus_function_adds_set_instance_builtin() -> Result<(), Box<dyn std::error::Error>>
{
    let mut executor = WasmExecutor::prepare_test(
        &[
            InstDD::PushInt(2),
            InstDD::PushInt(3),
            InstDD::Add,
            InstDD::Return,
        ],
        &[],
    );
    executor.set_instance(0);
    assert_eq!(executor.call_test_func(), 5);
    Ok(())
}
