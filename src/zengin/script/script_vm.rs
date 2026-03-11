use std::collections::{HashMap, VecDeque};

use bevy::ecs::error::{BevyError, Result};
use bevy::log::warn;

use crate::zengin::script::parse::{SymbolInstance, SymbolString};
use crate::{
    warn_unimplemented,
    zengin::script::parse::{DatFile, Function, Instance, Instruction, Symbol},
};

#[derive(Debug, Clone)]
pub enum StoredValue {
    Int(u32),
    Index(u32),
    IntArr(Vec<u32>),
    IndexArr(Vec<u32>),
}

impl StoredValue {
    pub fn get_int(&self) -> Result<u32> {
        let StoredValue::Int(int) = self else {
            return Err(BevyError::from(format!(
                "StoredValue({:?}) is not an int",
                self,
            )));
        };
        return Ok(*int);
    }
    pub fn get_index(&self) -> Result<u32> {
        let StoredValue::Index(int) = self else {
            return Err(BevyError::from(format!(
                "StoredValue({:?}) is not an int",
                self,
            )));
        };
        return Ok(*int);
    }

    // pub fn get_int_or_index(&self) -> Result<u32> {
    //     if let StoredValue::Index(int) = self {
    //         return Ok(*int);
    //     }
    //     if let StoredValue::Int(int) = self {
    //         return Ok(*int);
    //     }
    //     return Err(BevyError::from(format!(
    //         "StoredValue({:?}) is not an int or index",
    //         self,
    //     )));
    // }
}
#[derive(Debug, Clone)]
pub struct VmValue(pub i32);

#[derive(Debug, Clone)]
pub enum VariableRef {
    GlobalVar(u32),
    ClassVar(u32),
    GlobalArrVar(u32, u32),
    ClassArrVar(u32, u32),
}

#[derive(Debug, Clone)]
pub enum StackVVV {
    VmValue(VmValue),
    VariableRef(VariableRef),
}

#[derive(Debug, Default)]
pub struct RoutineEntry {
    pub start_h: u32,
    pub stop_h: u32,
    pub way_point: String,
}

#[derive(Debug, Default)]
pub struct InstanceState {
    pub body_model: String,
    pub body_texture: Option<String>,
    pub head_model: Option<String>,
    pub face_texture: Option<String>,
    pub armor_model: Option<String>,

    pub routine_enties: Vec<RoutineEntry>,
}

impl InstanceState {
    pub fn get_routine_entry(&self, hour: u32) -> Option<&RoutineEntry> {
        if self.routine_enties.is_empty() {
            return None;
        }
        for entry in &self.routine_enties {
            let in_range = if entry.stop_h < entry.start_h {
                hour > entry.start_h || hour < entry.stop_h
            } else {
                hour > entry.start_h && hour < entry.stop_h
            };

            if in_range {
                return Some(entry);
            }
        }

        return Some(&self.routine_enties[0]);
    }
}

#[derive(Debug, Default)]
pub struct SpawnNpc {
    pub npc_index: u32,
    pub way_point: String,
}

#[derive(Debug, Default)]
pub struct SpawnItem {
    pub visual: String,
    pub way_point: String,
}

#[derive(Debug, Default)]
pub struct ClassData {
    pub name: String,
    pub data: HashMap<u32, StoredValue>,
}
#[derive(Debug, Default)]
pub struct State {
    pub stack: VecDeque<StackVVV>,
    pub spawn_npcs: Vec<SpawnNpc>,
    pub spawn_weapons: Vec<SpawnItem>,
    pub instance_data: HashMap<u32, InstanceState>,
    pub class_instance_data: HashMap<u32, ClassData>,
    pub global_data: HashMap<u32, StoredValue>,
    pub current_instance: Option<u32>,
}

impl State {
    pub fn new() -> Self {
        State::default()
    }

    pub fn set_data(&mut self, offset: u32, arr_index: Option<u32>, value: StoredValue) {
        let Some(instance_name) = &self.current_instance else {
            println!(
                "Can't assign to class member variable as there is no current class in context, value({:?})",
                value
            );
            return;
        };

        let class = self.class_instance_data.entry(*instance_name).or_default();
        if let Some(arr_index) = arr_index {
            insert_arr_value_to_store(&mut class.data, offset, arr_index, value);
            return;
        }
        class.data.insert(offset, value);
    }

    pub fn get_value_inner(&self, offset: u32) -> Option<StoredValue> {
        let Some(instance_name) = &self.current_instance else {
            println!("Can't get class member variable as there is no current class in context");
            return None;
        };

        let Some(class_data) = self.class_instance_data.get(instance_name) else {
            println!("Class instance({instance_name}) is not present");
            return None;
        };

        let Some(value) = class_data.data.get(&offset) else {
            // println!("Class instance({instance_name}) doesn't contain offset({offset})");
            return None;
        };
        return Some(value.clone());
    }

    pub fn get_value(&mut self, var: &StackVVV) -> Option<StoredValue> {
        match var {
            StackVVV::VmValue(val) => {
                return Some(StoredValue::Int(val.0 as u32));
            }
            StackVVV::VariableRef(var_ref) => match var_ref {
                VariableRef::ClassVar(offset_in_class) => {
                    return self.get_value_inner(*offset_in_class);
                }
                VariableRef::GlobalVar(var_index) => {
                    return self.global_data.get(var_index).cloned();
                }
                VariableRef::GlobalArrVar(var_index, in_var_index) => {
                    let arr = self.global_data.get(var_index)?;
                    let StoredValue::IntArr(arr) = arr else {
                        println!("Tried to get array value from data which is not an array");
                        return None;
                    };
                    let value = arr.get(*in_var_index as usize).unwrap();
                    return Some(StoredValue::Int(*value));
                }
                VariableRef::ClassArrVar(offset_in_class, in_var_index) => {
                    let arr = self.get_value_inner(*offset_in_class)?;
                    let StoredValue::IntArr(arr) = arr else {
                        println!("Tried to get array value from data which is not an array");
                        return None;
                    };
                    let value = arr.get(*in_var_index as usize)?;
                    return Some(StoredValue::Int(*value));
                }
            },
        }
    }

    pub fn pop_stack_var(&mut self) -> Result<StackVVV> {
        let Some(world_point_name_index) = self.stack.pop_back() else {
            return Err(BevyError::from("There is no var on stack to pop"));
        };
        return Ok(world_point_name_index);
    }

    pub fn pop_stack_value(&mut self) -> Result<StoredValue> {
        let var = self.pop_stack_var()?;
        // println!("pop_stack_value: ({:?})", var);
        let Some(value) = self.get_value(&var) else {
            return Err(BevyError::from(format!(
                "Failed to get value from stack var({:?})",
                var,
            )));
        };
        return Ok(value);
    }

    pub fn pop_stack_var_int(&mut self) -> Result<u32> {
        let val = self.pop_stack_value()?;
        let int = val.get_int()?;

        return Ok(int);
    }

    pub fn pop_stack_var_index(&mut self) -> Result<u32> {
        let val = self.pop_stack_value()?;
        if let StoredValue::Index(index) = val {
            return Ok(index);
        }
        if let StoredValue::Int(index) = val {
            return Ok(index);
        }
        return Err(BevyError::from(format!(
            "StoredValue({:?}) is not an index",
            val,
        )));
    }

    // pub fn pop_stack_symbol(&self, state: &mut State) -> Result<&Symbol> {
    //     let index = self.pop_stack_var_index(state)?;
    //     let symbol = self
    //         .script_data
    //         .get_symbol_by_index(index)
    //         .ok_or(BevyError::from("Failed get_symbol_by_index"))?;
    //     return Ok(symbol);
    // }

    pub fn set_value(&mut self, dest_var: VariableRef, val: StoredValue) {
        match dest_var {
            VariableRef::GlobalVar(var_index) => {
                self.global_data.insert(var_index, val);
            }
            VariableRef::GlobalArrVar(var_index, in_var_index) => {
                insert_arr_value_to_store(&mut self.global_data, var_index, in_var_index, val);
            }
            VariableRef::ClassVar(class_offset) => {
                self.set_data(class_offset, None, val);
            }

            VariableRef::ClassArrVar(class_offset, in_var_index) => {
                self.set_data(class_offset, Some(in_var_index), val);
            }
        }
    }
    pub fn handle_assign_instruction(&mut self, is_instance: bool) {
        let Some(StackVVV::VariableRef(dest_var)) = self.stack.pop_back() else {
            println!("handle_assign_instruction expected Var on stack");
            return;
        };
        let Some(source_var) = self.stack.pop_back() else {
            println!("handle_assign_instruction expected Var on stack");
            return;
        };

        if is_instance
            && let StackVVV::VariableRef(VariableRef::GlobalVar(source_value)) = source_var
        {
            // println!("assign1 to({dest_var:?}) = ({source_value:?})");
            self.set_value(dest_var, StoredValue::Index(source_value));
            return;
        }
        let Some(source_value) = self.get_value(&source_var) else {
            // println!("handle_assign_instruction failed to get value");
            return;
        };
        // println!(
        //     "assign to({dest_var:?}) = ({source_value:?}), current_instance({:?})",
        //     self.current_instance
        // );
        self.set_value(dest_var, source_value);
    }
}

pub struct ScriptVM {
    pub script_data: DatFile,
}

fn insert_arr_value_to_store(
    store: &mut HashMap<u32, StoredValue>,
    offset: u32,
    in_var_index: u32,
    val: StoredValue,
) {
    match val {
        StoredValue::Int(val) => {
            let entry = store.entry(offset).or_insert(StoredValue::IntArr(vec![]));

            let arr = if let StoredValue::IntArr(arr) = entry {
                arr
            } else if let StoredValue::Int(var) = entry.clone() {
                // println!("Upgrade to array");
                store.insert(offset, StoredValue::IntArr(vec![var]));
                if let StoredValue::IntArr(arr) = store.get_mut(&offset).unwrap() {
                    arr
                } else {
                    panic!();
                }
            } else {
                panic!(
                    "Not handled insert to array entry({:?}) in_var_index({})",
                    entry, in_var_index
                );
            };

            if (in_var_index as usize) >= arr.len() {
                // println!("AAAADD TO ARR INDEX DEFAULT");
                for _ in 0..=((in_var_index as usize) - arr.len()) {
                    arr.push(u32::MAX);
                }
            }
            // println!("set var_index({offset}[{in_var_index}]) = {val}");
            arr[in_var_index as usize] = val;
        }
        StoredValue::Index(val) => {
            let entry = store.entry(offset).or_insert(StoredValue::IndexArr(vec![]));
            let StoredValue::IndexArr(arr) = entry else {
                panic!();
            };

            if (in_var_index as usize) >= arr.len() {
                // println!("AAAADD TO ARR INDEX DEFAULT");
                for _ in 0..=((in_var_index as usize) - arr.len()) {
                    arr.push(u32::MAX);
                }
            }
            // println!("set var_index({offset}[{in_var_index}]) = {val}");
            arr[in_var_index as usize] = val;
        }
        StoredValue::IntArr(_) | StoredValue::IndexArr(_) => {
            panic!("Should not set array in array")
        }
    }
}

impl ScriptVM {
    pub fn new(script_data: DatFile) -> Self {
        warn_unimplemented!("Scripts support is hacked to just somehow spawn NPCs");
        ScriptVM { script_data }
    }

    pub fn initialize_variables(&self, state: &mut State) {
        for (index, symbol) in self.script_data.symbols.iter().enumerate() {
            if let Some(val) = get_symbol_value(symbol, index as u32) {
                let dest_var = VariableRef::GlobalVar(index as u32);
                state.set_value(dest_var, val.clone());
            }
        }
        // let xardas = 11156;
        for instance in &self.script_data.instances {
            // if instance.symbol.name != "itmw_zweihaender1" {
            //     continue;
            // }
            // println!("instance: {:?}", instance.symbol.name);
            // if instance.symbol_table_index != xardas {
            //     continue;
            // }
            if !state
                .class_instance_data
                .contains_key(&(instance.symbol_table_index as u32))
            {
                self.interpret_instance(state, instance);
            }
        }
    }

    pub fn interpret_script_function(&self, state: &mut State, func_name: &str) {
        // println!("Interpret func_name({func_name})");
        let function = self.script_data.get_function(func_name).unwrap();
        self.interpret_function(state, function);
        // println!("Interpret func_name({func_name}) ENDDDD");
    }

    pub fn interpret_instructions(&self, state: &mut State, instructions: &[Instruction]) {
        for instruction in instructions {
            if let Instruction::Call(func_offset) = instruction {
                // println!("call func({})", func_offset);
                let Some(call_func) = self.script_data.get_function_by_offset(*func_offset) else {
                    // println!("Skipped call to func_offset({})", *func_offset);
                    continue;
                };
                self.interpret_function(state, call_func);
            }
            if let Instruction::CallExternal(func_offset) = instruction {
                self.interpret_external_function(state, *func_offset);
            } else if let Instruction::PushInt(var) = instruction {
                // println!("pushi ({:?})", var);
                state.stack.push_back(StackVVV::VmValue(VmValue(*var)));
            } else if let Instruction::PushVar(var) = instruction {
                // println!("pushv ({:?})", var);
                self.handle_push_var(state, *var, None);
            } else if let Instruction::PushInstance(var) = instruction {
                // println!("push_instance ({:?})", var);
                self.handle_push_var(state, *var, None);
            } else if let Instruction::PushArrayVar(var, index) = instruction {
                // println!("push_array_var ({:?}[{}])", var, index);
                self.handle_push_var(state, *var, Some(*index));
            } else if let Instruction::Assign = instruction {
                state.handle_assign_instruction(false);
            } else if let Instruction::AssignString = instruction {
                state.handle_assign_instruction(false);
            } else if let Instruction::AssignFunc = instruction {
                state.handle_assign_instruction(false);
                // panic!();
            } else if let Instruction::AssignFloat = instruction {
                // panic!();
            } else if let Instruction::AssignInstance = instruction {
                // println!("Assign Instance:");
                state.handle_assign_instruction(true);
                // panic!();
            } else if let Instruction::Return = instruction {
                // println!("<- return");
                break;
            } else if let Instruction::SetInstance(instance) = instruction {
                let source_var = StackVVV::VariableRef(VariableRef::GlobalVar(*instance));
                let Some(source_value) = state.get_value(&source_var) else {
                    continue;
                };
                if let Ok(index) = source_value.get_index() {
                    state.current_instance = Some(index);
                    // println!("Set Instance({:?})", instruction);
                } else {
                    println!("Failed to set instance({:?})", instruction);
                }
            }
        }
    }

    // pub fn interpret_instance_by_str(&self, state: &mut State, instance_name: &str) {
    //     let instance = self.script_data.get_instance(instance_name).unwrap();

    //     if !state
    //         .class_instance_data
    //         .contains_key(&(instance.symbol_table_index as u32))
    //     {
    //         self.interpret_instance(state, instance);
    //     }
    // }

    pub fn interpret_instance(&self, state: &mut State, instance: &Instance) {
        // println!("Interpret Instance({})", instance.symbol.name);
        assert_eq!(state.current_instance, None);
        let previous_instance = state.current_instance;
        state.current_instance = Some(instance.symbol_table_index as u32);
        let class = state
            .class_instance_data
            .entry(instance.symbol_table_index as u32)
            .or_default();
        class.name = instance.symbol.name.clone();

        self.interpret_instructions(state, &instance.instructions);

        let class = state
            .class_instance_data
            .entry(instance.symbol_table_index as u32)
            .or_default();

        // Interpret routines immediatelly as they contain data where NPC stands at given hour
        let daily_routine_func_offset = 608;
        if let Some(var) = class.data.get(&daily_routine_func_offset) {
            let index = var.get_int().unwrap();
            if let Some(func) = self.script_data.get_function_by_index(index) {
                self.interpret_function(state, func);
            }
        }

        // println!("Interpret Instance({}) END", instance.symbol.name);
        state.current_instance = previous_instance;
    }
    pub fn interpret_function(&self, state: &mut State, func: &Function) {
        assert!(!func.symbol.external);
        // println!("-> Interpret Func({})", func.symbol.name);
        let previous_instance = state.current_instance;
        self.interpret_instructions(state, &func.instructions);
        state.current_instance = previous_instance;
    }

    pub fn pop_stack_string(&self, state: &mut State) -> Result<SymbolString> {
        let index = state.pop_stack_var_index()?;
        let symbol = self
            .script_data
            .get_symbol_by_index(index)
            .ok_or(BevyError::from("Failed get_symbol_by_index"))?;
        let Symbol::SymbolString(symbol) = symbol else {
            return Err(BevyError::from(format!(
                "Symbol({:?}) is not a SymbolString",
                symbol,
            )));
        };
        return Ok(symbol.clone());
    }

    pub fn pop_stack_instance(&self, state: &mut State) -> Result<(u32, SymbolInstance)> {
        let index = state.pop_stack_var_index()?;
        let symbol = self
            .script_data
            .get_symbol_by_index(index)
            .ok_or(BevyError::from("Failed get_symbol_by_index"))?;
        let Symbol::SymbolInstance(symbol) = symbol else {
            return Err(BevyError::from(format!(
                "Symbol({:?}) is not a SymbolInstance",
                symbol,
            )));
        };
        return Ok((index, symbol.clone()));
    }

    pub fn interpret_external_function(&self, state: &mut State, func_offset: u32) {
        let symbol = self.script_data.get_symbol_by_index(func_offset).unwrap();

        if symbol.name() == "wld_insertnpc" {
            self.handle_wld_insertnpc(state);
            return;
        }
        if symbol.name() == "createinvitem" {
            Self::handle_createinvitem(state).unwrap();
            return;
        }
        if symbol.name() == "createinvitems" {
            Self::handle_createinvitems(state).unwrap();
            return;
        }
        if symbol.name() == "npc_changeattribute" {
            Self::handle_npc_changeattribute(state).unwrap();
            return;
        }
        if symbol.name() == "npc_isdead" {
            Self::handle_npc_isdead(state).unwrap();
            return;
        }
        if symbol.name() == "playvideo" {
            Self::handle_playvideo(state).unwrap();
            return;
        }
        if symbol.name() == "hlp_getnpc" {
            Self::handle_hlp_getnpc(state).unwrap();
            return;
        }
        if symbol.name() == "hlp_isvalidnpc" {
            Self::handle_hlp_isvalidnpc(state).unwrap();
            return;
        }
        if symbol.name() == "wld_insertitem" {
            self.handle_wld_insertitem(state).unwrap();
            return;
        }
        if symbol.name() == "mdl_setvisual" {
            Self::handle_mdl_setvisual(state).unwrap();
            return;
        }
        if symbol.name() == "mdl_setvisualbody" {
            self.handle_mdl_setvisualbody(state).unwrap();
            return;
        }
        if symbol.name() == "mdl_setmodelscale" {
            Self::handle_mdl_setmodelscale(state).unwrap();
            return;
        }
        if symbol.name() == "mdl_setmodelfatness" {
            Self::handle_mdl_setmodelfatness(state).unwrap();
            return;
        }
        if symbol.name() == "mdl_applyoverlaymds" {
            Self::handle_mdl_applyoverlaymds(state).unwrap();
            return;
        }
        if symbol.name() == "mdl_applyoverlaymdstimed" {
            Self::handle_mdl_applyoverlaymdstimed(state).unwrap();
            return;
        }
        if symbol.name() == "npc_settalentskill" {
            Self::handle_npc_settalentskill(state).unwrap();
            return;
        }
        if symbol.name() == "equipitem" {
            Self::handle_equipitem(state).unwrap();
            return;
        }
        if symbol.name() == "hlp_random" {
            Self::handle_hlp_random(state).unwrap();
            return;
        }
        if symbol.name() == "ta_min" {
            self.handle_ta_min(state).unwrap();
            return;
        }
        if symbol.name() == "npc_settofistmode" {
            Self::handle_npc_settofistmode(state).unwrap();
            return;
        }
        if symbol.name() == "npc_settofightmode" {
            Self::handle_npc_settofightmode(state).unwrap();
            return;
        }
        if symbol.name() == "inttostring" {
            Self::handle_inttostring(state).unwrap();
            return;
        }
        if symbol.name() == "npc_setattitude" {
            Self::handle_npc_setattitude(state).unwrap();
            return;
        }
        if symbol.name() == "npc_settempattitude" {
            Self::handle_npc_settempattitude(state).unwrap();
            return;
        }
        if symbol.name() == "concatstrings" {
            Self::handle_concatstrings(state).unwrap();
            return;
        }

        println!(
            "Script External Function({})({}) not implemented",
            func_offset,
            symbol.name()
        );
    }

    pub fn handle_push_var(&self, state: &mut State, var_index: u32, arr_index: Option<u8>) {
        let symbol = self.script_data.get_symbol_by_index(var_index).unwrap();
        match symbol {
            Symbol::SymbolInt(_) | Symbol::SymbolString(_) | Symbol::SymbolInstance(_) => {
                state
                    .stack
                    .push_back(StackVVV::VariableRef(VariableRef::GlobalVar(var_index)));
            }
            Symbol::SymbolArrInt(_) => {
                state
                    .stack
                    .push_back(StackVVV::VariableRef(VariableRef::GlobalArrVar(
                        var_index,
                        u32::from(arr_index.unwrap_or(0)),
                    )));
            }
            Symbol::SymbolClassVariable(var) => {
                if let Some(arr_index) = arr_index {
                    state
                        .stack
                        .push_back(StackVVV::VariableRef(VariableRef::ClassArrVar(
                            var_index,
                            u32::from(arr_index),
                        )));
                    return;
                }
                state
                    .stack
                    .push_back(StackVVV::VariableRef(VariableRef::ClassVar(
                        var.in_class_offset,
                    )));
            }

            Symbol::SymbolFloat(_)
            | Symbol::SymbolArrFloat(_)
            | Symbol::SymbolArrString(_)
            | Symbol::SymbolFunc(_)
            | Symbol::SymbolArrFunc(_)
            | Symbol::SymbolClass(_)
            | Symbol::SymbolPrototype(_)
            | Symbol::SymbolVariableArgument(_) => {
                warn!("not handled symbol type({:?}) in pushv instruction", symbol);
            }
        }
    }
}

pub fn get_symbol_value(symbol: &Symbol, var_index: u32) -> Option<StoredValue> {
    match symbol {
        Symbol::SymbolInt(var) => {
            return Some(StoredValue::Int(var.data));
        }
        Symbol::SymbolArrInt(var) => {
            if var.arr.is_empty() {
                return None;
            }
            return Some(StoredValue::Int(var.arr[0]));
        }
        Symbol::SymbolString(_var) => {
            return Some(StoredValue::Index(var_index));
        }

        Symbol::SymbolFloat(_)
        | Symbol::SymbolClassVariable(_)
        | Symbol::SymbolArrFloat(_)
        | Symbol::SymbolArrString(_)
        | Symbol::SymbolFunc(_)
        | Symbol::SymbolArrFunc(_)
        | Symbol::SymbolClass(_)
        | Symbol::SymbolInstance(_)
        | Symbol::SymbolPrototype(_)
        | Symbol::SymbolVariableArgument(_) => {
            // warn!(
            //     "get_symbol_value not handled symbol type({:?}) in pushv instruction",
            //     symbol
            // );
        }
    }
    return None;
}
