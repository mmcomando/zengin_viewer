use std::collections::HashMap;

use crate::{
    warn_unimplemented,
    zengin::script::parse::{DatFile, Symbol},
};

const INVALID_VALUE: u32 = u32::MAX;

#[derive(Debug, Clone, Copy)]
pub struct MemValue {
    val: u32,
}

impl MemValue {
    pub fn from(num: u32) -> Self {
        Self { val: num }
    }
    pub fn get_int(self) -> u32 {
        self.val
    }
    pub fn set_int(&mut self, val: u32) {
        self.val = val;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MemRef {
    pub id: u32,
    pub offset: Option<u32>,
    pub arr_index: Option<u8>,
}

impl MemRef {
    pub fn from(id: u32, offset: Option<u32>, arr_index: Option<u8>) -> Self {
        MemRef {
            id,
            offset,
            arr_index,
        }
    }
    pub fn global(id: u32) -> Self {
        MemRef {
            id,
            offset: None,
            arr_index: None,
        }
    }
    pub fn global_arr(id: u32, arr_index: u8) -> Self {
        MemRef {
            id,
            offset: None,
            arr_index: Some(arr_index),
        }
    }
    pub fn class(id: u32, offset: u32) -> Self {
        MemRef {
            id,
            offset: Some(offset),
            arr_index: None,
        }
    }
    // pub fn class_arr(id: u32, offset: u32, arr_index: u8) -> Self {
    //     MemRef {
    //         id,
    //         offset: Some(offset),
    //         arr_index: Some(arr_index),
    //     }
    // }
}

#[derive(Debug, Default)]
pub struct ScriptMem {
    mem: HashMap<u32, Vec<MemValue>>,
}

impl ScriptMem {
    pub fn from(file_data: &DatFile) -> Self {
        let mut mem = Self {
            mem: HashMap::new(),
        };

        for (id, symbol) in file_data.symbols.iter().enumerate() {
            let id = id as u32;
            match symbol {
                Symbol::SymbolInt(var) => {
                    mem.set_int(MemRef::global(id), var.data);
                }
                Symbol::SymbolArrInt(var) => {
                    assert!(var.arr.len() <= (u8::MAX as usize) + 1);
                    for (arr_index, el) in var.arr.iter().enumerate() {
                        let arr_index = arr_index as u8;
                        mem.set_int(MemRef::global_arr(id, arr_index), *el);
                    }
                }
                Symbol::SymbolString(_) | Symbol::SymbolFunc(_) | Symbol::SymbolClass(_) => {
                    mem.set_int(MemRef::global(id), id);
                }
                Symbol::SymbolArrString(var) => {
                    assert!(u8::try_from(var.arr.len()).is_ok());
                    for (arr_index, _el) in var.arr.iter().enumerate() {
                        let arr_index = arr_index as u8;
                        mem.set_int(MemRef::global_arr(id, arr_index), id);
                    }
                }
                Symbol::SymbolArrFunc(var) => {
                    assert!(u8::try_from(var.arr.len()).is_ok());
                    for (arr_index, _el) in var.arr.iter().enumerate() {
                        let arr_index = arr_index as u8;
                        mem.set_int(MemRef::global_arr(id, arr_index), id);
                    }
                }
                // This don't contain any data, so there is nothing to initialize
                Symbol::SymbolClassVariable(_) |

                // This are initialized using scripts
                Symbol::SymbolInstance(_) | Symbol::SymbolPrototype(_) => {}

                Symbol::SymbolFloat(_) | Symbol::SymbolArrFloat(_) => {
                    warn_unimplemented!("Floats in scripts not supported");
                }
                Symbol::SymbolVariableArgument(_) => {
                    println!("Init memory for symbol({id}) type({:?})", symbol);
                }
            }
        }
        mem
    }

    pub fn get_value(&self, mem_ref: MemRef) -> MemValue {
        let Some(mem_under_id) = self.mem.get(&mem_ref.id) else {
            println!("There is no id under ref({mem_ref:?}), returning({INVALID_VALUE})");
            return MemValue::from(INVALID_VALUE);
        };
        let mut value_index = 0;
        if let Some(offset) = mem_ref.offset {
            assert!(offset % 4 == 0, "All ZenGin values have 4 bytes");
            value_index = offset / 4;
        }
        if let Some(arr_index) = mem_ref.arr_index {
            value_index += u32::from(arr_index);
        }
        let value_index = value_index as usize;
        if let Some(val) = mem_under_id.get(value_index) {
            return *val;
        }
        println!("There is no value under ref({mem_ref:?}), returning({INVALID_VALUE})");
        MemValue::from(INVALID_VALUE)
    }

    pub fn get_int(&self, mem_ref: MemRef) -> u32 {
        self.get_value(mem_ref).get_int()
    }

    pub fn id_exists(&self, id: u32) -> bool {
        self.mem.contains_key(&id)
    }

    pub fn set_int(&mut self, mem_ref: MemRef, val: u32) {
        if mem_ref.id > 100_000 {
            println!(
                "Invalid memory set, mem_ref({:?})={val}, id should be less than 100_000",
                mem_ref
            );
            return;
        }
        let mem_under_id = self.mem.entry(mem_ref.id).or_default();
        let mut value_index = 0;
        if let Some(offset) = mem_ref.offset {
            assert!(offset % 4 == 0, "All ZenGin values have 4 bytes");
            value_index = offset / 4;
        }
        if let Some(arr_index) = mem_ref.arr_index {
            value_index += u32::from(arr_index);
        }
        let value_index = value_index as usize;
        if value_index > 11000 {
            println!(
                "Invalid memory set, mem_ref({:?})={val}, value_index should be less than 11_000",
                mem_ref
            );
            return;
        }

        ensure_arr_lenght(mem_under_id, value_index + 1);
        mem_under_id[value_index].set_int(val);
    }
}

fn ensure_arr_lenght(arr: &mut Vec<MemValue>, len: usize) {
    let prev_len = arr.len();
    if arr.len() >= len {
        return;
    }

    arr.extend((arr.len()..len).map(|_| MemValue::from(INVALID_VALUE)));
    if prev_len == 0 {
        assert_eq!(arr.len(), len);
    }
}
