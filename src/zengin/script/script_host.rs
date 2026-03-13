use bevy::ecs::error::Result;

use crate::{
    warn_unimplemented,
    zengin::script::{
        memory::MemRef,
        script_vm::{RoutineEntry, ScriptVM, SpawnItem, SpawnNpc, State},
    },
};

impl ScriptVM {
    pub fn handle_mdl_setvisualbody(&self, state: &mut State) -> Result {
        let armor_var = state.pop_stack_var_int();
        let _par_1 = state.pop_stack_var()?;
        let face_texture_index = state.pop_stack_var_int()?;
        let head_model = self.pop_stack_string(state)?;
        let _par_4 = state.pop_stack_var()?;
        let body_texture_index = state.pop_stack_var_int()?;
        let body_model = self.pop_stack_string(state)?;
        let npc_index = state.pop_stack_var_int();

        // mdl_setvisualbody is called twice in b_setnpcvisual because we don't handle 'if' in scripts
        // Most models are male so ignore female bodies for now
        if body_model == "hum_body_babe0" {
            return Ok(());
        }

        let face_texture = if face_texture_index != 0 {
            Some(format!("HUM_HEAD_V{}_C0.TGA", face_texture_index))
        } else {
            None
        };
        let body_texture = if body_texture_index != 0 {
            Some(format!("HUM_BODY_NAKED_V{}_C0.TGA", body_texture_index))
        } else {
            None
        };
        let head_model = if head_model.is_empty() {
            None
        } else {
            // Sometimes there is model "Hum_Head_Babe." (dot at the end), engine or scripts bug?
            Some(head_model.replace('.', ""))
        };

        let armor_model = if let Ok(armor_var) = &armor_var {
            self.get_string(*armor_var).ok()
        } else {
            None
        };

        let npc_index = if let Ok(npc_index) = npc_index {
            npc_index
        } else if let Some(npc_index) = state.current_instance {
            npc_index
        } else {
            println!("No instance for mdl_setvisualbody");
            return Ok(());
        };

        let entry = state.instance_data.entry(npc_index).or_default();

        entry.body_texture = body_texture;
        entry.face_texture = face_texture;
        entry.body_model.clone_from(body_model);
        entry.head_model = head_model;
        entry.armor_model = armor_model.cloned();

        return Ok(());
    }

    pub fn handle_ta_min(&self, state: &mut State) -> Result {
        let way_point = self.pop_stack_string(state)?.clone();
        let _func_index = state.pop_stack_var()?;
        let _stop_m = state.pop_stack_var_int()?;
        let stop_h = state.pop_stack_var_int()?;
        let _start_m = state.pop_stack_var_int()?;
        let start_h = state.pop_stack_var_int()?;
        let _npc_index = state.pop_stack_var()?; // Fix this should have npc index

        let instance = state.current_instance.unwrap();

        let entry = state.instance_data.entry(instance).or_default();

        let routine_entry = RoutineEntry {
            start_h,
            stop_h,
            way_point,
        };

        entry.routine_enties.push(routine_entry);

        return Ok(());
    }

    pub fn handle_wld_insertitem(&self, state: &mut State) -> Result {
        let way_point_name = self.pop_stack_string(state)?;
        let item_index = state.pop_stack_var_int()?;

        let visual_offset = 524;
        let wepon_visual_index = state.mem.get_int(MemRef::class(item_index, visual_offset));
        if wepon_visual_index == 0 {
            println!("Weapon visual_offset is not set for item({item_index})");
            return Ok(());
        }
        let Ok(wepon_string) = self.get_string(wepon_visual_index) else {
            println!("Weapon({visual_offset}) visual_offset not found on instance({item_index})");
            return Ok(());
        };
        // println!(
        //     "wld_insertitem way_point({:?}), www({:?})",
        //     way_point_name.data, wepon_string.data
        // );

        state.spawn_weapons.push(SpawnItem {
            visual: wepon_string.clone(),
            way_point: way_point_name.clone(),
        });

        return Ok(());
    }

    pub fn handle_wld_insertnpc(&self, state: &mut State) -> Result {
        let point_symbol = self.pop_stack_string(state)?;
        let Ok(npc_symbol_index) = state.pop_stack_var_int() else {
            println!("world_point_name_index should point to instance type");
            return Ok(());
        };

        // println!(
        //     "Spawn npc({})({npc_symbol_index}) on pos({})",
        //     npc_symbol.name, point_symbol.data
        // );

        state.spawn_npcs.push(SpawnNpc {
            // npc: npc_symbol.name.clone(),
            npc_index: npc_symbol_index,
            way_point: point_symbol.clone(),
        });
        Ok(())
    }
}
