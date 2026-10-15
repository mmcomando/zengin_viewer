mod common;
mod conversions;
mod dialog;
mod instance_index;
mod interaction;
mod loaders;
mod macros;
mod npc_routines;
mod objects_to_entities;
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
mod debug_zengin_scripts;

use bevy::prelude::*;

use crate::zengin::script::symbol_id::InstanceId;

// Expose only what is needed for outside modules
pub use crate::zengin::loaders::vdf_reader::{create_zengin_asset_loader, create_zengin_vfs};
pub use crate::zengin::sounds::SpeechSounds;
pub use crate::zengin::statistics::StatisticsZengin;
pub use crate::zengin::waypoint_graph::WaypointDebugPlugin;

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
        app.add_plugins(crate::zengin::statistics::StatisticsZenginPlugin);
        app.add_plugins(crate::zengin::script::component::GothicScriptsPlugin);
        app.add_plugins(crate::zengin::dialog::GameDialogPlugin);
    }
}

/// Identifies Entity with given Zengin script instance id
#[derive(Debug, Component)]
pub struct GameNpc {
    npc_instance_id: InstanceId,
}

/// Identifies Entity with a Zengin script item instance id
#[derive(Debug, Component)]
pub struct GameItem {
    item_instance_id: InstanceId,
}
