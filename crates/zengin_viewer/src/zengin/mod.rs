mod common;
pub(crate) mod dialog;
mod fight_ai;
mod instance_index;
mod interaction;
mod loaders;
pub(crate) use crate::zengin::common::choose_animation_scheme;
pub(crate) use crate::zengin::interaction::{grant_test_items, seed_test_journal};
pub(crate) use crate::zengin::loaders::vdf_reader::get_zengin_asset_bytes;
pub(crate) use crate::zengin::npc_routines::{AnimationFiles, NpcOverlay};
mod item_data;
mod item_visual;
mod npc_actions;
mod npc_data;
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

use crate::world_entity::WorldEntityMarker;

// Expose only what is needed for outside modules
pub(crate) use crate::zengin::common::MusicZones;
pub(crate) use crate::zengin::common::face_direction_quat;
pub(crate) use crate::zengin::common::get_animation_path;
pub(crate) use crate::zengin::common::get_full_texture_path;
pub(crate) use crate::zengin::common::gothic_dat_path;
pub(crate) use crate::zengin::common::gothic2_dir;
pub(crate) use crate::zengin::common::to_world_units;
pub(crate) use crate::zengin::dialog::DialogRequest;
pub(crate) use crate::zengin::dialog::DialogUiState;
pub(crate) use crate::zengin::dialog::ImportantDialogOptions;
pub(crate) use crate::zengin::dialog::SpeechSounds;
pub(crate) use crate::zengin::fight_ai::{FightAiResource, load_fight_ai_tables};
pub(crate) use crate::zengin::interaction::MeleeHit;
pub(crate) use crate::zengin::interaction::ranged::ArrowShot;
pub(crate) use crate::zengin::interaction::selection::{Selection, SelectionKind};
pub(crate) use crate::zengin::item_data::ItemHandle;
pub(crate) use crate::zengin::loaders::vdf_reader::{
    ZenginVfs, create_zengin_asset_loader, create_zengin_vfs,
};
pub(crate) use crate::zengin::loaders::vfs::{resolve_sound_asset, resolve_texture_asset};
pub(crate) use crate::zengin::npc_actions::NpcActionQueue;
pub(crate) use crate::zengin::npc_data::NpcHandle;
pub(crate) use crate::zengin::npc_routines::NpcHealth;
pub(crate) use crate::zengin::npc_routines::NpcWeaponMode;
pub(crate) use crate::zengin::resources::CachedAssetHandles;
pub(crate) use crate::zengin::resources::NPC_BODY;
pub(crate) use crate::zengin::resources::STATIC_OBJECT;
pub(crate) use crate::zengin::script::bridge::ScriptChangesApplied;
pub(crate) use crate::zengin::script::component::ScriptResource;
pub(crate) use crate::zengin::script::journal::{JournalSection, JournalStatus, JournalTopic};
pub(crate) use crate::zengin::script::manager::ItemDisplayData;
pub(crate) use crate::zengin::script::manager::ScriptManager;
pub(crate) use crate::zengin::script::state::ChapterIntro;
pub(crate) use crate::zengin::script::state::ScreenMessage;
pub(crate) use crate::zengin::script::state::ScriptSound;
pub(crate) use crate::zengin::script::state::WeaponKind;
pub(crate) use crate::zengin::script::state::WeaponMode;
pub(crate) use crate::zengin::script::state::{ItemRuntime, MobRoutineEntry, NpcRuntime};
pub(crate) use crate::zengin::script::symbol_id::InstanceId;
pub(crate) use crate::zengin::script::{
    MENU_FLAG_ALIGN_CENTER, MENU_FLAG_DONT_SCALE_DIM, MENU_FLAG_SHOW_INFO, MENU_ITEM_FLAG_DISABLED,
    MENU_ITEM_FLAG_TXT_CENTER, MENU_SCRIPT_DIV, MenuDefinition, MenuItemDefinition,
    MenuScriptResource, SEL_ACTION_BACK, SEL_ACTION_CLOSE, SEL_ACTION_START_MENU,
    ensure_menu_script,
};
pub(crate) use crate::zengin::statistics::StatisticsZengin;
pub(crate) use crate::zengin::waypoint_graph::WaypointGraph;
pub(crate) use crate::zengin::world_plugin::{
    CurrentWorld, LoadingWorld, NewGame, RestartWorld, begin_world_load, request_initial_world,
};

/// Plugin which provides Zengin related functionality
pub struct ZenginPlugin;

impl Plugin for ZenginPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(crate::zengin::world_plugin::ZenGinWorldPlugin);
        app.add_plugins(crate::zengin::npc_routines::NpcRoutinesPlugin);
        app.add_plugins(crate::zengin::interaction::selection::SelectionPlugin);
        app.add_plugins(crate::zengin::instance_index::InstanceIndexPlugin);
        app.add_plugins(crate::zengin::script::bridge::ScriptBridgePlugin);
        app.add_plugins(crate::zengin::item_visual::ItemVisualPlugin);
        app.add_plugins(crate::zengin::interaction::PlayerInteractionPlugin);
        app.add_plugins(crate::zengin::interaction::ranged::RangedCombatPlugin);
        app.add_plugins(crate::zengin::statistics::StatisticsZenginPlugin);
        app.add_plugins(crate::graphics::AnimationTriggersPlugin);
        app.add_plugins(crate::zengin::script::component::GothicScriptsPlugin);
        app.add_plugins(crate::zengin::dialog::GameDialogPlugin);
    }
}

/// Identifies Entity with the shared data of a Zengin script npc instance.
#[derive(Debug, Component, Clone)]
#[require(WorldEntityMarker)]
pub struct GameNpc {
    handle: NpcHandle,
}

impl GameNpc {
    pub fn new(handle: NpcHandle) -> Self {
        Self { handle }
    }

    /// Script instance id of this npc.
    pub fn instance_id(&self) -> InstanceId {
        self.handle.instance_id()
    }

    /// Shared typed and runtime data of this npc.
    pub fn handle(&self) -> &NpcHandle {
        &self.handle
    }
}

/// Identifies Entity with the shared data of a Zengin script item instance.
#[derive(Debug, Component, Clone)]
#[require(WorldEntityMarker)]
pub struct GameItem {
    handle: ItemHandle,
}

impl GameItem {
    pub fn new(handle: ItemHandle) -> Self {
        Self { handle }
    }

    /// Script instance id of this item.
    pub fn instance_id(&self) -> InstanceId {
        self.handle.instance_id()
    }

    /// Shared typed and runtime data of this item.
    pub fn handle(&self) -> &ItemHandle {
        &self.handle
    }
}

/// Interactive chest vob with its remaining loot.
#[derive(Debug, Component)]
#[require(WorldEntityMarker)]
pub struct GameChest {
    /// Position of the container in the ZEN load order, used to match saved
    /// loot to the same chest.
    pub index: usize,
    pub name: String,
    /// Vob `locked` flag, reported when the chest is opened.
    pub locked: bool,
    /// Remaining contents as `(item, count)` pairs.
    pub items: Vec<(InstanceId, i32)>,
}
