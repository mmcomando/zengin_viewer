use wasmtime::AsContextMut;

use super::bindings::mmcomando;
use super::manager::ScriptImports;
use super::state::{DialogLine, DialogRoot, ImportCallState, RoutineEntry, SpawnItem, SpawnNpc};
use super::symbol_id::{InstanceId, SymbolId};
use crate::zengin::common::to_zengin_units;

impl ScriptImports {
    fn get_string_non_0(
        access: &mut wasmtime::component::Access<'_, ImportCallState, ScriptImports>,
        string_id: SymbolId,
    ) -> Option<String> {
        let world = access.get().world.clone().unwrap();
        let store_context = access.as_context_mut();
        let string = world
            .mmcomando_zengine_scripts_manage()
            .call_get_name(store_context, string_id.value())
            .unwrap();
        // .ok()?;
        Some(string)
    }

    /// Binds the given instance for an engine driven invocation of its
    /// instance function: sets the current instance context and binds it as
    /// `self`, like the original engine does when it initializes an instance
    /// (zenkit `DaedalusVm::init_instance`), then invokes the instance's init
    /// function.
    fn init_instance(
        access: &mut wasmtime::component::Access<'_, ImportCallState, ScriptImports>,
        instance_id: InstanceId,
    ) {
        let world = access.get().world.clone().unwrap();
        let store_context = access.as_context_mut();
        let guest = world.mmcomando_zengine_scripts_manage();
        guest
            .call_set_self(store_context, instance_id.value())
            .unwrap();
        let store_context = access.as_context_mut();
        guest
            .call_set_instance(store_context, instance_id.value())
            .unwrap();
        let store_context = access.as_context_mut();
        guest
            .call_call_function(store_context, instance_id.value())
            .unwrap();
    }
}

impl mmcomando::zengine_scripts::scripts::HostWithStore<ImportCallState> for ScriptImports {
    fn wld_insertnpc(
        mut access: wasmtime::component::Access<'_, ImportCallState, ScriptImports>,
        npc_index: i32,
        waypoint_string_id: i32,
    ) {
        let Some(npc_index) = access.data_mut().symbol_table.instance(npc_index) else {
            return;
        };
        let Some(waypoint_string_id) = access.data_mut().symbol_table.string(waypoint_string_id)
        else {
            return;
        };
        Self::init_instance(&mut access, npc_index);
        // let npc_name = Self::get_string_non_0(&mut access, npc_index.symbol()).unwrap();
        let waypoint_name = Self::get_string_non_0(&mut access, waypoint_string_id.symbol())
            .unwrap()
            .to_lowercase();

        // println!(
        //     "NEW wld_insertnpc npc_index({npc_index}), npc_name({npc_name}), waypoint_string_id({waypoint_string_id}), waypoint_name({waypoint_name})"
        // );

        let npc = SpawnNpc {
            npc_index,
            way_point: waypoint_name,
        };
        let state = access.data_mut();
        state.loaded_state.spawn_npcs.push(npc);
    }

    fn wld_insertitem(
        mut access: wasmtime::component::Access<'_, ImportCallState, ScriptImports>,
        item_index: i32,
        waypoint_string_id: i32,
    ) {
        // println!("CCC");
        let Some(item_index) = access.data_mut().symbol_table.instance(item_index) else {
            return;
        };
        let Some(waypoint_string_id) = access.data_mut().symbol_table.string(waypoint_string_id)
        else {
            return;
        };
        Self::init_instance(&mut access, item_index);
        let item_name = Self::get_string_non_0(&mut access, item_index.symbol()).unwrap();
        let waypoint_name = Self::get_string_non_0(&mut access, waypoint_string_id.symbol())
            .unwrap()
            .to_ascii_lowercase();
        // println!(
        //     "NEW wld_insertitem item_index({item_index}), item_name({item_name}), waypoint_string_id({waypoint_string_id}), waypoint_name({waypoint_name})"
        // );

        let state = access.data_mut();
        state.loaded_state.spawn_items.push(SpawnItem {
            instance_name: item_name,
            way_point: waypoint_name,
        });
    }

    fn ta_min(
        mut access: wasmtime::component::Access<'_, ImportCallState, ScriptImports>,
        npc_index: i32,
        start_h: i32,
        start_m: i32,
        stop_h: i32,
        stop_m: i32,
        func_index: i32,
        way_point: i32,
    ) {
        // println!(
        //     "NEW ta_min npc_index({npc_index}), start_h({start_h}), stop_h({stop_h}), way_point({way_point})"
        // );
        let Some(npc_index) = access.data_mut().symbol_table.instance(npc_index) else {
            return;
        };
        let Some(way_point) = access.data_mut().symbol_table.string(way_point) else {
            return;
        };
        let way_point = Self::get_string_non_0(&mut access, way_point.symbol())
            .unwrap()
            .to_lowercase();

        let routine_entry = RoutineEntry {
            start_h: start_h.max(0) as u32,
            start_m: start_m.max(0) as u32,
            stop_h: stop_h.max(0) as u32,
            stop_m: stop_m.max(0) as u32,
            way_point,
            // Id 0 means the state has no function
            state_fn: access.data_mut().symbol_table.function(func_index),
        };

        let state = access.data_mut();
        let entry = state
            .loaded_state
            .instance_data
            .entry(npc_index)
            .or_default();
        entry.routine_enties.push(routine_entry);
    }

    fn ai_playani(
        mut access: wasmtime::component::Access<'_, ImportCallState, ScriptImports>,
        npc_index: i32,
        ani_id: i32,
    ) {
        let Some(npc_index) = access.data_mut().symbol_table.instance(npc_index) else {
            return;
        };
        let Some(ani_id) = access.data_mut().symbol_table.string(ani_id) else {
            return;
        };
        let Some(ani) = Self::get_string_non_0(&mut access, ani_id.symbol()) else {
            return;
        };
        let state = access.data_mut();
        state
            .loaded_state
            .routine_played_anis
            .entry(npc_index)
            .or_default()
            .push(ani);
    }

    fn ai_playanibs(
        mut access: wasmtime::component::Access<'_, ImportCallState, ScriptImports>,
        npc_index: i32,
        ani_id: i32,
        _body_state: i32,
    ) {
        let Some(npc_index) = access.data_mut().symbol_table.instance(npc_index) else {
            return;
        };
        let Some(ani_id) = access.data_mut().symbol_table.string(ani_id) else {
            return;
        };
        let Some(ani) = Self::get_string_non_0(&mut access, ani_id.symbol()) else {
            return;
        };
        let state = access.data_mut();
        state
            .loaded_state
            .routine_played_anis
            .entry(npc_index)
            .or_default()
            .push(ani);
    }

    fn ai_output(
        mut access: wasmtime::component::Access<'_, ImportCallState, ScriptImports>,
        npc_a: i32,
        npc_b: i32,
        line_id: i32,
    ) {
        let Some(npc_a) = access.data_mut().symbol_table.instance(npc_a) else {
            return;
        };
        let Some(npc_b) = access.data_mut().symbol_table.instance(npc_b) else {
            return;
        };
        let Some(line_id) = access.data_mut().symbol_table.string(line_id) else {
            return;
        };
        let line_str = Self::get_string_non_0(&mut access, line_id.symbol())
            .unwrap()
            .to_lowercase();
        // println!(
        //     "ai_output npc_a({npc_a}), npc_b({npc_b}), line_id({line_id}), line_str({line_str})"
        // );
        let state = access.data_mut();
        state.dialog_state.dialog_lines.push(DialogLine {
            npc_a,
            npc_b,
            line_id,
            line_str,
        });
    }

    fn ai_usemob(
        mut access: wasmtime::component::Access<'_, ImportCallState, ScriptImports>,
        npc_index: i32,
        mob_name_id: i32,
        state: i32,
    ) -> i32 {
        // Negative states release the mob, nothing to animate.
        if state <= 0 {
            return 0;
        }
        let Some(npc_index) = access.data_mut().symbol_table.instance(npc_index) else {
            return 0;
        };
        let Some(mob_name_id) = access.data_mut().symbol_table.string(mob_name_id) else {
            return 0;
        };
        let Some(mob) = Self::get_string_non_0(&mut access, mob_name_id.symbol()) else {
            return 0;
        };
        let ani = format!("S_{}_S{state}", mob.to_uppercase());
        let data = access.data_mut();
        data.loaded_state
            .routine_played_anis
            .entry(npc_index)
            .or_default()
            .push(ani);
        0
    }

    fn info_addchoice(
        mut access: wasmtime::component::Access<'_, ImportCallState, ScriptImports>,
        _dialog_id: i32,
        description_text_id: i32,
        function_id: i32,
    ) {
        let Some(description_text_id) = access.data_mut().symbol_table.string(description_text_id)
        else {
            return;
        };
        let Some(function_id) = access.data_mut().symbol_table.function(function_id) else {
            return;
        };
        let description =
            Self::get_string_non_0(&mut access, description_text_id.symbol()).unwrap_or_default();

        let state = access.data_mut();
        state.dialog_state.sub_dialog_choices.insert(
            0,
            DialogRoot {
                info_instance_id: None,
                sequence_function_id: function_id,
                condition_function_id: None,
                order: state.dialog_state.sub_dialog_choices.len() as i32,
                description,
                important: false,
                permanent: false,
            },
        );
    }

    /// Returns whether the two string symbols hold equal text (Daedalus
    /// `Hlp_StrCmp` is an equality test, not an ordering comparison).
    fn hlp_strcmp(
        mut access: wasmtime::component::Access<'_, ImportCallState, ScriptImports>,
        a: i32,
        b: i32,
    ) -> i32 {
        let Some(a) = access.data_mut().symbol_table.string(a) else {
            return 0;
        };
        let Some(b) = access.data_mut().symbol_table.string(b) else {
            return 0;
        };
        let a = Self::get_string_non_0(&mut access, a.symbol());
        let b = Self::get_string_non_0(&mut access, b.symbol());
        (a == b) as i32
    }

    /// Distance from the npc to a named waypoint in engine units. The engine
    /// returns `i32::MAX` when the npc or the waypoint is unknown.
    fn npc_getdisttowp(
        mut access: wasmtime::component::Access<'_, ImportCallState, ScriptImports>,
        npc: i32,
        wp_name: i32,
    ) -> i32 {
        let Some(npc) = access.data_mut().symbol_table.instance(npc) else {
            return i32::MAX;
        };
        let Some(wp_name) = access.data_mut().symbol_table.string(wp_name) else {
            return i32::MAX;
        };
        let Some(wp_name) = Self::get_string_non_0(&mut access, wp_name.symbol()) else {
            return i32::MAX;
        };
        let wp_name = wp_name.to_lowercase();

        let data = access.get();
        let Some(target) = data.way_points.get(&wp_name).copied() else {
            return i32::MAX;
        };
        let Some(state) = data.npc_live_state() else {
            return i32::MAX;
        };
        let Some(position) = state.positions.get(&npc).copied() else {
            return i32::MAX;
        };

        // Positions are stored in Bevy meters, the engine works in game units
        let distance = to_zengin_units((position - target).length());
        distance.min(i32::MAX as f32) as i32
    }
}
