use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct RoutineEntry {
    pub start_h: u32,
    pub start_m: u32,
    pub stop_h: u32,
    pub stop_m: u32,
    pub way_point: String,
    /// Symbol index of the routine state function (`ZS_*`), `0` when not available.
    pub state_fn: u32,
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
    /// Routine entry active at `minutes_of_day`, matching the engine semantics:
    /// entries start (inclusive) at their start time and stop (exclusive) at
    /// their stop time, entries wrapping over midnight stay active past it.
    pub fn get_routine_entry(&self, minutes_of_day: u32) -> Option<&RoutineEntry> {
        let first = self.routine_enties.first()?;
        let minutes = minutes_of_day % (24 * 60);
        for entry in &self.routine_enties {
            let start = entry.start_h * 60 + entry.start_m;
            let stop = entry.stop_h * 60 + entry.stop_m;
            let in_range = if stop <= start {
                minutes >= start || minutes < stop
            } else {
                minutes >= start && minutes < stop
            };

            if in_range {
                return Some(entry);
            }
        }

        return Some(first);
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

#[derive(Debug, Default, Clone)]
pub struct DialogRoot {
    /// Symbol index of the `C_INFO` instance this root was loaded from,
    /// `0` for sub dialog choices registered via `Info_AddChoice`
    pub info_instance_id: u32, // TODO make optional or make Info_AddChoice a separate type
    pub sequence_function_id: i32,
    pub condition_function_id: i32,
    pub order: i32,
    pub description: String,
    pub important: bool,
    pub permanent: bool,
}

#[derive(Debug, Default)]
pub struct LoadedState {
    pub spawn_npcs: Vec<SpawnNpc>,
    pub spawn_items: Vec<SpawnItem>,
    pub instance_data: HashMap<u32, InstanceState>,
    pub item_instances: HashMap<String, ItemInstance>,
    pub dialog_roots: HashMap<u32, Vec<DialogRoot>>,
    /// Animations requested via `AI_PlayAni(BS)` by the last executed routine
    /// state function, keyed by npc instance index, in request order.
    pub routine_played_anis: HashMap<u32, Vec<String>>,
}

#[derive(Debug, Default, Clone)]
pub struct DialogLine {
    pub npc_a: i32,
    pub npc_b: i32,
    pub line_id: i32,
    pub line_str: String,
}

#[derive(Debug, Default)]
pub struct DialogState {
    pub dialog_lines: Vec<DialogLine>,
    pub close_dialog: bool,
    pub sub_dialog_choices: Vec<DialogRoot>,
}
