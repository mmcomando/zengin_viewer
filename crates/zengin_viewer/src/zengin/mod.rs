pub mod common;
pub mod conversions;
pub mod dialog;
pub mod loaders;
pub mod macros;
pub mod npc_routines;
pub mod objects_to_entities;
pub mod script;
pub mod sounds;
pub mod statistics;
pub mod visual;
pub mod world;
pub mod world_plugin;
pub mod zengin_resources;

// Used to debug data, it is more like a tool than a test
#[cfg(test)]
mod debug_zengin_scripts;

use bevy::ecs::component::Component;

use crate::zengin::script::symbol_id::InstanceId;

/// Identifies the npc's script instance.
#[derive(Debug, Component)]
pub struct GameNpc {
    npc_instance_id: InstanceId,
}

#[derive(Default, Component)]
pub struct PlayerNpc;
