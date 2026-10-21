mod animation_graph_nodes;
pub use crate::zengin::animation_graph_nodes::{AnimationGraph, AnimationIndex};
pub mod animation_triggers;
mod common;
mod conversions;
pub(crate) mod dialog;
mod instance_index;
mod interaction;
mod loaders;
mod macros;
mod npc_routines;
mod objects_to_entities;
mod ranged;
mod script;
mod script_bridge;
mod selection;
mod sounds;
mod statistics;
mod visual;
mod waypoint_graph;
mod world;
mod world_plugin;
mod zengin_resources;

// Used to debug data, it is more like a tool than a test
#[cfg(test)]
mod animation_graph_export;
#[cfg(test)]
mod debug_zengin_scripts;

use bevy::prelude::*;

// Expose only what is needed for outside modules
pub use crate::zengin::animation_triggers::animation_name_from_path;
pub use crate::zengin::common::face_direction_quat;
pub use crate::zengin::common::get_animation_path;
pub use crate::zengin::interaction::MeleeHit;
pub use crate::zengin::loaders::vdf_reader::{
    ZenginVfs, create_zengin_asset_loader, create_zengin_vfs,
};
pub use crate::zengin::npc_routines::NpcHealth;
pub use crate::zengin::npc_routines::NpcWeaponMode;
pub use crate::zengin::ranged::ArrowShot;
pub use crate::zengin::script::component::ScriptResource;
pub use crate::zengin::script::state::WeaponKind;
pub use crate::zengin::script::state::WeaponMode;
pub use crate::zengin::script::symbol_id::InstanceId;
pub use crate::zengin::selection::{Selection, SelectionKind};
pub use crate::zengin::sounds::SpeechSounds;
pub use crate::zengin::statistics::StatisticsZengin;
pub use crate::zengin::waypoint_graph::WaypointGraph;
pub use crate::zengin::zengin_resources::MaterialHandles;
pub use crate::zengin::zengin_resources::NPC_BODY;
pub use crate::zengin::zengin_resources::STATIC_OBJECT;

/// Plugin which provides Zengin related functionality
pub struct ZenginPlugin;

impl Plugin for ZenginPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(crate::zengin::world_plugin::ZenGinWorldPlugin);
        app.add_plugins(crate::zengin::npc_routines::NpcRoutinesPlugin);
        app.add_plugins(crate::zengin::selection::SelectionPlugin);
        app.add_plugins(crate::zengin::instance_index::InstanceIndexPlugin);
        app.add_plugins(crate::zengin::script_bridge::ScriptBridgePlugin);
        app.add_plugins(crate::zengin::interaction::PlayerInteractionPlugin);
        app.add_plugins(crate::zengin::ranged::RangedCombatPlugin);
        app.add_plugins(crate::zengin::statistics::StatisticsZenginPlugin);
        app.add_plugins(crate::zengin::animation_triggers::AnimationTriggersPlugin);
        app.add_plugins(crate::zengin::script::component::GothicScriptsPlugin);
        app.add_plugins(crate::zengin::dialog::GameDialogPlugin);
    }
}

/// Identifies Entity with given Zengin script instance id
#[derive(Debug, Component)]
pub struct GameNpc {
    npc_instance_id: InstanceId,
}

impl GameNpc {
    /// Script instance id of this npc.
    pub fn instance_id(&self) -> InstanceId {
        self.npc_instance_id
    }
}

/// Identifies Entity with a Zengin script item instance id
#[derive(Debug, Component)]
pub struct GameItem {
    item_instance_id: InstanceId,
}
