mod common;
pub(crate) mod dialog;
mod instance_index;
mod interaction;
mod loaders;
pub(crate) use crate::zengin::common::choose_animation_scheme;
pub(crate) use crate::zengin::interaction::{grant_test_items, seed_test_journal};
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
pub(crate) use crate::zengin::common::face_direction_quat;
pub(crate) use crate::zengin::common::get_animation_path;
pub(crate) use crate::zengin::common::get_full_texture_path;
pub(crate) use crate::zengin::dialog::DialogUiState;
pub(crate) use crate::zengin::dialog::SpeechSounds;
pub(crate) use crate::zengin::interaction::MeleeHit;
pub(crate) use crate::zengin::interaction::ranged::ArrowShot;
pub(crate) use crate::zengin::interaction::selection::{Selection, SelectionKind};
pub(crate) use crate::zengin::loaders::vdf_reader::{
    ZenginVfs, create_zengin_asset_loader, create_zengin_vfs,
};
pub(crate) use crate::zengin::npc_routines::NpcHealth;
pub(crate) use crate::zengin::npc_routines::NpcWeaponMode;
pub(crate) use crate::zengin::resources::CachedAssetHandles;
pub(crate) use crate::zengin::resources::NPC_BODY;
pub(crate) use crate::zengin::resources::STATIC_OBJECT;
pub(crate) use crate::zengin::script::component::ScriptResource;
pub(crate) use crate::zengin::script::journal::{JournalSection, JournalStatus, JournalTopic};
pub(crate) use crate::zengin::script::manager::ItemDisplayData;
pub(crate) use crate::zengin::script::manager::ScriptManager;
pub(crate) use crate::zengin::script::state::ScreenMessage;
pub(crate) use crate::zengin::script::state::WeaponKind;
pub(crate) use crate::zengin::script::state::WeaponMode;
pub(crate) use crate::zengin::script::symbol_id::InstanceId;
pub(crate) use crate::zengin::statistics::StatisticsZengin;
pub(crate) use crate::zengin::waypoint_graph::WaypointGraph;

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

/// Interactive chest vob with its remaining loot.
#[derive(Debug, Component)]
pub struct GameChest {
    pub name: String,
    /// Vob `locked` flag, reported when the chest is opened.
    pub locked: bool,
    /// Remaining contents as `(item, count)` pairs.
    pub items: Vec<(InstanceId, i32)>,
}
