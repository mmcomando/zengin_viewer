use std::collections::{HashMap, VecDeque};

use crate::zengin::script::{DatFile, Function, Instance, Instruction, Symbol, SymbolString};

pub enum StackVar {
    Int(i32),
    Var(u32),
    Instance(u32),
}

#[derive(Debug, Default)]
pub struct InstanceState {
    pub body_texture: String,
    pub face_texture: String,
    pub head_model: String,
}
pub struct SpawnNpc {
    pub npc_name: String,
    pub way_point_name: String,
    pub routine_way_point_name: Option<String>,
}
pub struct State {
    pub stack: VecDeque<StackVar>,
    pub spawn_npcs: Vec<SpawnNpc>,
    pub instance_state: HashMap<String, InstanceState>,
    pub current_instance: Option<String>,
}
impl State {
    pub fn new() -> Self {
        State {
            stack: VecDeque::new(),
            spawn_npcs: Vec::new(),
            instance_state: HashMap::new(),
            current_instance: None,
        }
    }
}

pub struct ScriptVM {
    script_data: DatFile,
}
impl ScriptVM {
    pub fn new(script_data: DatFile) -> Self {
        ScriptVM { script_data }
    }
    pub fn interpret_script_function(&self, state: &mut State, func_name: &str) {
        println!("\n\nInterpret func_name({func_name})");
        let function = self.script_data.get_function(func_name).unwrap();
        self.interpret_function(state, &function);
    }

    fn interpret_instructions(&self, state: &mut State, instructions: &[Instruction]) {
        for instruction in instructions {
            if let Instruction::Call(func_offset) = instruction {
                // println!("call func({})", func_offset);
                let Some(call_func) = self.script_data.get_function_by_offset(*func_offset) else {
                    println!("Skipped call to func_offset({})", *func_offset);
                    continue;
                };
                self.interpret_function(state, call_func);
            }
            if let Instruction::CallExternal(func_offset) = instruction {
                // println!("call func({})", func_offset);
                self.interpret_external_function(state, *func_offset);
            }
            if let Instruction::PushInt(var) = instruction {
                state.stack.push_back(StackVar::Int(*var));
            }
            if let Instruction::PushVar(var) = instruction {
                state.stack.push_back(StackVar::Var(*var));
                if *var == 1494 {
                    // c_npc.daily_routine
                    self.handle_set_daily_rutine(state);
                }
            }
            if let Instruction::PushInstance(var) = instruction {
                state.stack.push_back(StackVar::Instance(*var));
            }
        }
    }
    fn interpret_instance(&self, state: &mut State, instance: &Instance) {
        println!("Interpret Instance({})", instance.symbol.name);
        state.current_instance = Some(instance.symbol.name.clone());
        self.interpret_instructions(state, &instance.instructions);
        state.stack.clear();
        state.current_instance = None;
    }
    fn interpret_function(&self, state: &mut State, func: &Function) {
        assert!(!func.symbol.external);
        if func.symbol.name.to_lowercase() == "b_setnpcvisual" {
            self.handle_b_setnpcvisual(state);
            return;
        }

        if func.symbol.name.to_lowercase() == "ta_stand_armscrossed" {
            self.handle_routine_waypoint(state);
            return;
        }

        println!("Interpret Func({})", func.symbol.name);
        self.interpret_instructions(state, &func.instructions);
        state.stack.clear();
    }

    fn interpret_external_function(&self, state: &mut State, func_offset: u32) {
        let symbol = self.script_data.get_symbol_by_index(func_offset).unwrap();
        if symbol.name() == "WLD_INSERTNPC" {
            self.handle_wld_insertnpc(state);
            return;
        }
        // println!(
        //     "Script External Function({})({}) not implemented",
        //     func_offset,
        //     symbol.name()
        // );
        state.stack.clear();
    }

    fn handle_routine_waypoint(&self, state: &mut State) {
        let Some(StackVar::Var(waypoin_index)) = state.stack.pop_back() else {
            println!("handle_routine_waypoint expected Var on stack");
            return;
        };
        let waypoint = self
            .script_data
            .get_symbol_by_index(waypoin_index as u32)
            .unwrap();
        let Symbol::SymbolString(waypoint) = waypoint else {
            println!("handle_routine_waypoint routine should point to SymbolString type");
            return;
        };

        let Some(current_instance) = &state.current_instance else {
            println!("handle_routine_waypoint current_instance shoudl be set");
            return;
        };

        let Some(npc_data) = state
            .spawn_npcs
            .iter_mut()
            .find(|el| el.npc_name.to_lowercase() == current_instance.to_lowercase())
        else {
            println!(
                "handle_routine_waypoint npc_data for({}) should be present",
                current_instance
            );
            return;
        };
        println!(
            "Set routine waypoint({}) for npc({})",
            waypoint.data, current_instance
        );
        npc_data.routine_way_point_name = Some(waypoint.data.clone());
    }

    fn handle_set_daily_rutine(&self, state: &mut State) {
        let Some(StackVar::Var(_obj_var_index)) = state.stack.pop_back() else {
            println!("handle_set_daily_rutine expected Var on stack");
            return;
        };
        let Some(StackVar::Int(routine_function_index)) = state.stack.pop_back() else {
            println!("handle_set_daily_rutine expected Int on stack");
            return;
        };
        let routine = self
            .script_data
            .get_symbol_by_index(routine_function_index as u32)
            .unwrap();
        let Symbol::SymbolFunc(routine) = routine else {
            println!("handle_set_daily_rutine routine should point to SymbolFunc type");
            return;
        };
        println!(
            "NPC({:?}) routine({})",
            state.current_instance, routine.name
        );
        self.interpret_script_function(state, &routine.name.to_lowercase());
    }

    fn handle_b_setnpcvisual(&self, state: &mut State) {
        let Some(StackVar::Int(_num)) = state.stack.pop_back() else {
            println!("handle_b_setnpcvisual expected Int on stack");
            return;
        };
        let Some(StackVar::Var(body_texture)) = state.stack.pop_back() else {
            println!("handle_b_setnpcvisual expected Var on stack");
            return;
        };
        let Some(StackVar::Var(face_texture)) = state.stack.pop_back() else {
            println!("handle_b_setnpcvisual expected Var on stack");
            return;
        };
        let Some(StackVar::Var(head_model)) = state.stack.pop_back() else {
            println!("handle_b_setnpcvisual expected Var on stack");
            return;
        };
        let Some(StackVar::Var(_gender)) = state.stack.pop_back() else {
            println!("handle_b_setnpcvisual expected Var on stack");
            return;
        };
        let Some(StackVar::Instance(npc_instance)) = state.stack.pop_back() else {
            println!("handle_b_setnpcvisual expected Instance on stack");
            return;
        };
        let npc_instance = self
            .script_data
            .get_symbol_by_index(npc_instance as u32)
            .unwrap();
        let Symbol::SymbolInstance(npc_instance) = npc_instance else {
            println!("handle_b_setnpcvisual npc_instance should point to SymbolInstance type");
            return;
        };
        let entry = state
            .instance_state
            .entry(npc_instance.name.clone())
            .or_default();

        let body_texture = self.script_data.get_symbol_by_index(body_texture).unwrap();
        let Symbol::SymbolInt(body_texture) = body_texture else {
            println!("handle_b_setnpcvisual body_texture should point to int type");
            return;
        };
        let face_texture = self.script_data.get_symbol_by_index(face_texture).unwrap();
        let Symbol::SymbolInt(face_texture) = face_texture else {
            println!("handle_b_setnpcvisual face_texture should point to int type");
            return;
        };
        let head_model = self.script_data.get_symbol_by_index(head_model).unwrap();
        let Symbol::SymbolString(head_model) = head_model else {
            println!("handle_b_setnpcvisual head_model should point to string type");
            return;
        };

        entry.body_texture = format!("HUM_BODY_NAKED_V{}_C0.TGA", body_texture.data);
        entry.face_texture = format!("HUM_HEAD_V{}_C0.TGA", face_texture.data);
        entry.head_model = head_model.data.clone();
        println!("handle_b_setnpcvisual => {:?}", entry);
    }

    fn handle_wld_insertnpc(&self, state: &mut State) {
        let Some(StackVar::Var(world_point_name_index)) = state.stack.pop_back() else {
            println!("wld_insertnpc expects world_point_name_index on stack");
            return;
        };
        let Some(StackVar::Int(npc_instance_index)) = state.stack.pop_back() else {
            println!("wld_insertnpc expects npc_instance_index on stack");
            return;
        };
        let point_symbol = self
            .script_data
            .get_symbol_by_index(world_point_name_index)
            .unwrap();
        let Symbol::SymbolString(point_symbol) = point_symbol else {
            println!("world_point_name_index should point to string type");
            return;
        };

        let npc_symbol = self
            .script_data
            .get_symbol_by_index(npc_instance_index as u32)
            .unwrap();
        let Symbol::SymbolInstance(npc_symbol) = npc_symbol else {
            println!("world_point_name_index should point to SymbolInstance type");
            return;
        };

        // if npc_symbol.name != "NONE_100_XARDAS" {
        //     return;
        // }
        println!(
            "Spawn npc({})({npc_instance_index}) on pos({})({world_point_name_index})",
            npc_symbol.name, point_symbol.data
        );
        let instance = self
            .script_data
            .get_instance(&npc_symbol.name.to_lowercase())
            .unwrap();

        state.spawn_npcs.push(SpawnNpc {
            npc_name: npc_symbol.name.clone(),
            way_point_name: point_symbol.data.clone(),
            routine_way_point_name: None,
        });

        if !state.instance_state.contains_key(&npc_symbol.name) {
            self.interpret_instance(state, instance);
        }
    }
}
