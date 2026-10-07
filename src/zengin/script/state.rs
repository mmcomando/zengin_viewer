use std::collections::HashMap;

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

#[derive(Debug, Default, Clone)]
pub struct DialogRoot {
    pub sequence_function_id: i32,
    pub order: i32,
    pub description: String,
    pub important: bool,
}

#[derive(Debug, Default)]
pub struct LoadedState {
    pub spawn_npcs: Vec<SpawnNpc>,
    pub spawn_items: Vec<SpawnItem>,
    pub instance_data: HashMap<u32, InstanceState>,
    pub item_instances: HashMap<String, ItemInstance>,
    pub dialog_roots: HashMap<u32, Vec<DialogRoot>>,
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
}
