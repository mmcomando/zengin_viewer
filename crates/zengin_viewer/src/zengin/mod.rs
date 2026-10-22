mod common;
pub(crate) mod dialog;
mod instance_index;
mod interaction;
mod loaders;
pub(crate) use crate::zengin::common::choose_animation_scheme;
pub(crate) use crate::zengin::loaders::vdf_reader::get_zengin_asset_bytes;
pub(crate) use crate::zengin::npc_routines::{AnimationFiles, NpcOverlay};
mod npc_routines;
mod objects_to_entities;
mod resources;
mod script;
mod statistics;
mod visual;
mod waypoint_graph;
mod world_plugin;

// Used to debug data, it is more like a tool than a test
#[cfg(test)]
mod animation_graph_export;

use bevy::prelude::*;

// Expose only what is needed for outside modules
pub use crate::zengin::common::face_direction_quat;
pub use crate::zengin::common::get_animation_path;
pub use crate::zengin::dialog::SpeechSounds;
pub use crate::zengin::interaction::MeleeHit;
pub use crate::zengin::interaction::ranged::ArrowShot;
pub use crate::zengin::interaction::selection::{Selection, SelectionKind};
pub use crate::zengin::loaders::vdf_reader::{
    ZenginVfs, create_zengin_asset_loader, create_zengin_vfs,
};
pub use crate::zengin::npc_routines::NpcHealth;
pub use crate::zengin::npc_routines::NpcWeaponMode;
pub use crate::zengin::resources::MaterialHandles;
pub use crate::zengin::resources::NPC_BODY;
pub use crate::zengin::resources::STATIC_OBJECT;
pub use crate::zengin::script::component::ScriptResource;
pub use crate::zengin::script::manager::ItemDisplayData;
pub use crate::zengin::script::state::WeaponKind;
pub use crate::zengin::script::state::WeaponMode;
pub use crate::zengin::script::symbol_id::InstanceId;
pub use crate::zengin::statistics::StatisticsZengin;
pub use crate::zengin::waypoint_graph::WaypointGraph;

/// Plugin which provides Zengin related functionality
pub struct ZenginPlugin;

impl Plugin for ZenginPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(crate::zengin::world_plugin::ZenGinWorldPlugin);
        app.add_plugins(crate::zengin::npc_routines::NpcRoutinesPlugin);
        app.add_plugins(crate::zengin::interaction::selection::SelectionPlugin);
        app.add_plugins(crate::zengin::instance_index::InstanceIndexPlugin);
        app.add_plugins(crate::zengin::script::bridge::ScriptBridgePlugin);
        app.add_plugins(crate::zengin::interaction::PlayerInteractionPlugin);
        app.add_plugins(crate::zengin::interaction::ranged::RangedCombatPlugin);
        app.add_plugins(crate::zengin::statistics::StatisticsZenginPlugin);
        app.add_plugins(crate::graphics::AnimationTriggersPlugin);
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
