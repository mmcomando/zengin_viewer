use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use bevy::ecs::error::{BevyError, Result};

use crate::println_vm;
use crate::zengin::script::memory::{MemRef, MemValue, ScriptMem};
use crate::zengin::script::parse::{Prototype, Symbol};
use crate::{
    warn_unimplemented,
    zengin::script::parse::{DatFile, Function, Instance, Instruction},
};

#[derive(Debug, Clone)]
pub enum StackVVV {
    MemValue(MemValue),
    MemRef(MemRef),
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
    pub hierarchy: Option<String>,

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
    pub instance_name: String,
    pub way_point: String,
}

#[derive(Debug, Default)]
pub struct ItemInstance {
    pub visual: String,
    pub visual_change: Option<String>,
}

#[derive(Debug)]
pub struct State {
    pub stack: VecDeque<StackVVV>,
    pub spawn_npcs: Vec<SpawnNpc>,
    pub spawn_weapons: Vec<SpawnItem>,
    pub instance_data: HashMap<u32, InstanceState>,
    pub item_instances: HashMap<String, ItemInstance>,
    pub current_instance: Option<u32>,
    pub mem: ScriptMem,
    pub script_data: Arc<DatFile>,
}

impl State {
    pub fn new(script_data: Arc<DatFile>) -> Self {
        let mem = ScriptMem::from(&script_data);
        State {
            stack: VecDeque::new(),
            spawn_npcs: Vec::new(),
            spawn_weapons: Vec::new(),
            instance_data: HashMap::new(),
            item_instances: HashMap::new(),
            current_instance: None,
            mem,
            script_data,
        }
    }

    // TODO fix lint
    #[allow(clippy::unnecessary_wraps)]
    pub fn get_value(&mut self, var: &StackVVV) -> Option<MemValue> {
        match var {
            StackVVV::MemValue(val) => {
                return Some(*val);
            }
            StackVVV::MemRef(var_ref) => {
                if var_ref.arr_index.is_none()
                    && var_ref.offset.is_none()
                    && self.script_data.is_instance_with_code(var_ref.id)
                {
                    // println!(
                    //     "get mem ret({}) instead of ({})",
                    //     var_ref.id,
                    //     self.mem.get_value(*var_ref).get_int()
                    // );
                    return Some(MemValue::from(var_ref.id));
                }
                return Some(self.mem.get_value(*var_ref));
            }
        }
    }

    pub fn pop_mem_ref(&mut self) -> Result<MemRef> {
        let var = self.pop_stack_var()?;
        let StackVVV::MemRef(mem_ref) = var else {
            return Err(BevyError::from(
                "Popped variable is not a variable reference",
            ));
        };
        return Ok(mem_ref);
    }
    pub fn pop_stack_var(&mut self) -> Result<StackVVV> {
        let Some(var) = self.stack.pop_back() else {
            return Err(BevyError::from("There is no var on stack to pop"));
        };
        // println!("pop_stack_var: {:?}  ({:?})", var, self.get_value(&var));
        return Ok(var);
    }

    pub fn pop_stack_value(&mut self) -> Result<MemValue> {
        let var = self.pop_stack_var()?;
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
        let int = val.get_int();
        return Ok(int);
    }

    pub fn push_stack_int(&mut self, val: u32) {
        // println!("push int: {val}");
        self.stack
            .push_back(StackVVV::MemValue(MemValue::from(val)));
    }

    pub fn push_stack_mem_ref(&mut self, mem_ref: MemRef) {
        // println!(
        //     "push ref: {:?}  ({:?})",
        //     mem_ref,
        //     self.get_value(&StackVVV::MemRef(mem_ref))
        // );
        self.stack.push_back(StackVVV::MemRef(mem_ref));
    }

    pub fn set_value(&mut self, dest_var: MemRef, val: MemValue) {
        self.mem.set_int(dest_var, val.get_int());
    }
    pub fn handle_assign_instruction(&mut self) -> Result {
        let dest_mem_loc = self.pop_mem_ref()?;
        let src_var = self.pop_stack_var()?;
        let value = self.get_value(&src_var).ok_or_else(|| {
            BevyError::from(format!("Failed to get value from src_var({:?})", src_var,))
        })?;

        // println!(
        //     "assign{} to({dest_mem_loc:?}) = ({value:?}), current_instance({:?}) ",
        //     if is_instance { " INSTANCE" } else { "" },
        //     self.current_instance
        // );
        self.set_value(dest_mem_loc, value);
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub enum ClassType {
    Npc,
    Item,
    Info,
}

pub struct ScriptVM {
    pub script_data: Arc<DatFile>,
}

impl ScriptVM {
    pub fn new(script_data: Arc<DatFile>) -> Self {
        warn_unimplemented!("Scripts support is hacked to just somehow spawn NPCs");
        ScriptVM { script_data }
    }

    pub fn initialize_variables(&self, state: &mut State) {
        for instance in &self.script_data.instances {
            // if instance.symbol.name != "none_100_xardas" {
            //     continue;
            // }
            // if instance.symbol.name != "pc_hero" {
            //     continue;
            // }

            if !state.mem.id_exists(instance.symbol_table_index)
                && self.get_type_by_index(instance.symbol_table_index) != Some(ClassType::Info)
            {
                self.interpret_instance(state, instance);
            }
        }
        self.instantiate_item_instances(state);
    }

    pub fn instantiate_item_instances(&self, state: &mut State) {
        for instance in &self.script_data.instances {
            let index = instance.symbol_table_index;
            if instance.symbol.instructions_offset == 0 {
                continue;
            }
            if !state.mem.id_exists(index) {
                continue;
            }
            if self.get_type_by_index(index) != Some(ClassType::Item) {
                continue;
            }

            let visual_offset = 524;

            let Some(visual) = self.get_string_from_var(state, MemRef::class(index, visual_offset))
            else {
                println!("Weapon({visual_offset}) visual_offset not found on instance({index})");
                continue;
            };
            let visual = visual.to_uppercase().replace(".3DS", "");

            let visual_change_offset = 544;
            let visual_change = self
                .get_string_from_var(state, MemRef::class(index, visual_change_offset))
                .map(|el| el.to_uppercase().replace(".3DS", ""));

            let item = ItemInstance {
                visual,
                visual_change,
            };
            // println!(
            //     "init item instance name({}) item({:?})",
            //     instance.symbol.name, item
            // );
            state
                .item_instances
                .insert(instance.symbol.name.clone(), item);
        }
    }

    pub fn instantiate_npc_routines(&self, state: &mut State) {
        for instance in &self.script_data.instances {
            self.instantiate_npc_routine(state, instance);
        }
    }
    pub fn instantiate_npc_routine(&self, state: &mut State, instance: &Instance) {
        let previous_instance = state.current_instance;
        state.current_instance = Some(instance.symbol_table_index);

        if self.get_type_by_index(instance.symbol_table_index) != Some(ClassType::Npc) {
            return;
        }
        if !state.mem.id_exists(instance.symbol_table_index) {
            return;
        }
        // Hardcoded routine handling
        let daily_routine_func_offset = 608;
        let index = state.mem.get_int(MemRef::class(
            instance.symbol_table_index,
            daily_routine_func_offset,
        ));
        if let Some(func) = self.script_data.get_function_by_index(index) {
            self.interpret_function(state, func);
        }
        state.current_instance = previous_instance;
    }

    pub fn interpret_script_function(&self, state: &mut State, func_name: &str) {
        let function = self.script_data.get_function(func_name).unwrap();
        self.interpret_function(state, function);
    }

    pub fn interpret_instructions(&self, state: &mut State, instructions: &[Instruction]) {
        for instruction in instructions {
            if let Instruction::Call(func_offset) = instruction {
                let Some(call_func) = self.script_data.get_function_by_offset(*func_offset) else {
                    // println!("Skipped call to func_offset({})", *func_offset);
                    continue;
                };
                self.interpret_function(state, call_func);
            }
            if let Instruction::CallExternal(func_offset) = instruction {
                self.interpret_external_function(state, *func_offset);
            } else if let Instruction::PushInt(var) = instruction {
                state.push_stack_int(*var as u32);
            } else if let Instruction::PushVar(var) = instruction {
                self.handle_push_var(state, *var, None);
            } else if let Instruction::PushInstance(var) = instruction {
                self.handle_push_var(state, *var, None);
            } else if let Instruction::PushArrayVar(var, index) = instruction {
                self.handle_push_var(state, *var, Some(*index));
            } else if let Instruction::Assign = instruction {
                state.handle_assign_instruction().unwrap();
            } else if let Instruction::AssignString = instruction {
                state.handle_assign_instruction().unwrap();
            } else if let Instruction::AssignFunc = instruction {
                state.handle_assign_instruction().unwrap();
            } else if let Instruction::AssignFloat = instruction {
                // panic!();
            } else if let Instruction::AssignInstance = instruction {
                state.handle_assign_instruction().unwrap();
            } else if let Instruction::Return = instruction {
                // println!("<- return");
                break;
            } else if let Instruction::SetInstance(instance_index) = instruction {
                let source_var = StackVVV::MemRef(MemRef::global(*instance_index));
                let Some(source_value) = state.get_value(&source_var) else {
                    continue;
                };
                state.current_instance = Some(source_value.get_int());
            }
        }
    }
    pub fn initialize_class_memory(&self, state: &mut State, class_index: u32) {
        let current_instance = state.current_instance.unwrap();
        // Initialize memory to 0
        if let Some(Symbol::SymbolClass(class)) = self.script_data.symbols.get(class_index as usize)
        {
            for index in 0..=class.size {
                state
                    .mem
                    .set_int(MemRef::class(current_instance, index * 4), 0);
            }
        }
    }

    pub fn interpret_prototype(&self, state: &mut State, prototype: &Prototype) {
        println_vm!("-> Interpret Prototype({:?})", prototype.symbol.name);
        self.initialize_class_memory(state, prototype.symbol.parent);
        if let Some(parent_prototype) = self
            .script_data
            .get_prototype_by_index(prototype.symbol.parent)
        {
            self.interpret_prototype(state, parent_prototype);
        }

        self.interpret_instructions(state, &prototype.instructions);
        println_vm!("<- Interpret Prototype({:?}) END", prototype.symbol.name);
    }

    pub fn get_type_by_index(&self, index: u32) -> Option<ClassType> {
        const ITEM_CLASS_INDEX: u32 = 1521;
        const NPC_CLASS_INDEX: u32 = 1474;
        const INFO_CLASS_INDEX: u32 = 1586;
        if index == NPC_CLASS_INDEX {
            return Some(ClassType::Npc);
        }
        if index == ITEM_CLASS_INDEX {
            return Some(ClassType::Item);
        }
        if index == INFO_CLASS_INDEX {
            return Some(ClassType::Info);
        }
        let symbol = self.script_data.symbols.get(index as usize)?;
        if let Some(parent) = symbol.parent() {
            return self.get_type_by_index(parent);
        }
        None
    }

    pub fn interpret_instance(&self, state: &mut State, instance: &Instance) {
        println_vm!(
            "Interpret Instance({})({})",
            instance.symbol.name,
            instance.symbol_table_index
        );

        assert_eq!(state.current_instance, None);
        state
            .mem
            .set_int(MemRef::global(1616), instance.symbol_table_index);
        let previous_instance = state.current_instance;
        state.current_instance = Some(instance.symbol_table_index);

        if let Some(parent) = instance.parent {
            self.initialize_class_memory(state, parent);
            if let Some(prototype) = self.script_data.get_prototype_by_index(parent) {
                self.interpret_prototype(state, prototype);
            }
        }
        self.interpret_instructions(state, &instance.instructions);

        state.current_instance = previous_instance;

        state.mem.set_int(MemRef::global(1616), 8_888_888);
        println_vm!("Interpret Instance({}) END", instance.symbol.name);
    }
    pub fn interpret_function(&self, state: &mut State, func: &Function) {
        assert!(!func.symbol.external);
        println_vm!("-> Interpret Func({})", func.symbol.name);
        let previous_instance = state.current_instance;
        self.interpret_instructions(state, &func.instructions);
        state.current_instance = previous_instance;
        println_vm!("<- Interpret Func({}) ENDD", func.symbol.name);
    }

    pub fn get_string(&self, index: u32) -> Result<&String> {
        self.script_data
            .strings
            .get(&index)
            .ok_or(BevyError::from(format!(
                "Failed to get string from index({index})"
            )))
    }

    pub fn get_string_from_var(&self, state: &mut State, mem_ref: MemRef) -> Option<&String> {
        let string_index = state.mem.get_int(mem_ref);
        if string_index == 0 {
            return None;
        }
        return self.get_string(string_index).ok();
    }

    pub fn pop_stack_string(&self, state: &mut State) -> Result<&String> {
        let index = state.pop_stack_var_int()?;
        self.get_string(index)
    }

    pub fn interpret_external_function(&self, state: &mut State, func_offset: u32) {
        let symbol = self.script_data.get_symbol_by_index(func_offset).unwrap();

        if symbol.name() == "wld_insertnpc" {
            self.handle_wld_insertnpc(state).unwrap();
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
            self.handle_mdl_setvisual(state).unwrap();
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

    pub fn handle_push_var(&self, state: &mut State, mut id: u32, arr_index: Option<u8>) {
        let class_offset = self.script_data.class_offsets.get(&id).copied();
        if class_offset.is_some() {
            id = state.current_instance.unwrap();
        }
        let mem_ref = MemRef::from(id, class_offset, arr_index);
        state.push_stack_mem_ref(mem_ref);
    }
}
