mod animation;
mod audio;
mod change_level;
mod common;
pub(crate) mod dialog;
mod fight_ai;
mod game_state;
pub(crate) mod gameplay;
pub(crate) mod gui;
mod instance_index;
mod interaction;
mod loaders;
mod mobs;
mod movers;
pub(crate) use crate::zengin::animation::{
    AnimatedJoint, AnimationController, AnimationData, AnimationEnds, AnimationGraph,
    AnimationSample, AnimationState, AnimationTriggers, AnimationTriggersPlugin, AutoAnimSequencer,
    AutoAnimSequencerSet, BoneAnimationData, DesiredAnimation, FrameState, MeleeAttackKind,
    NpcAnimations, NpcHitReaction, PlayerMeleeCombo, SetDesiredAnimation, SkeletalAnimations,
    animation_name_from_path, is_melee_attack_animation, npc_hit_reaction_end, play_hit_reaction,
};
pub(crate) use crate::zengin::common::choose_animation_scheme;
pub(crate) use crate::zengin::interaction::{NpcDied, grant_test_items, seed_test_journal};
pub(crate) use crate::zengin::loaders::vdf_reader::get_zengin_asset_bytes;
pub(crate) use crate::zengin::npc_routines::{
    AnimationFiles, NpcOverlay, NpcTalentOverlay, talent_overlay_for,
};
mod item_data;
mod item_visual;
mod npc_actions;
mod npc_corpse;
mod npc_data;
mod npc_perception;
mod npc_routines;
mod objects_to_entities;
mod resources;
pub(crate) mod save;
mod script;
mod statistics;
mod visual;
mod waypoint_graph;
mod world_entity;
mod world_plugin;
mod world_spawn;

use bevy::prelude::*;

// Expose only what is needed for outside modules
pub(crate) use crate::zengin::audio::{PendingMusic, PlaySound, StopSound};
pub(crate) use crate::zengin::common::MobPosition;
pub(crate) use crate::zengin::common::MusicZones;
pub(crate) use crate::zengin::common::animation_file_name;
pub(crate) use crate::zengin::common::face_direction_quat;
pub(crate) use crate::zengin::common::get_animation_path;
pub(crate) use crate::zengin::common::get_dialog_sound_path;
pub(crate) use crate::zengin::common::get_full_texture_path;
pub(crate) use crate::zengin::common::gothic_dat_path;
pub(crate) use crate::zengin::common::gothic2_dir;
pub(crate) use crate::zengin::common::to_world_units;
pub(crate) use crate::zengin::dialog::DialogRequest;
pub(crate) use crate::zengin::dialog::DialogUiState;
pub(crate) use crate::zengin::dialog::DialogVoice;
pub(crate) use crate::zengin::dialog::SpeechSounds;
pub(crate) use crate::zengin::fight_ai::{FightAiResource, load_fight_ai_tables};
pub(crate) use crate::zengin::game_state::{
    AppState, GameStatePlugin, LoadingScreenReady, loading_screen_ready, world_is_loaded,
};
pub(crate) use crate::zengin::interaction::MeleeHit;
pub(crate) use crate::zengin::interaction::ranged::ArrowShot;
pub(crate) use crate::zengin::interaction::selection::{Selection, SelectionKind};
pub(crate) use crate::zengin::item_data::ItemHandle;
pub(crate) use crate::zengin::loaders::vdf_reader::{
    ZenginVfs, create_zengin_asset_loader, create_zengin_vfs,
};
pub(crate) use crate::zengin::loaders::vfs::{resolve_sound_asset, resolve_texture_asset};
pub(crate) use crate::zengin::mobs::{MobHiddenModel, MobOccupant, MobRegistry, NpcMobUse};
pub(crate) use crate::zengin::movers::{GameMover, VobTrigger};
pub(crate) use crate::zengin::npc_actions::NpcActionQueue;
pub(crate) use crate::zengin::npc_actions::enqueue_npc_goto;
pub(crate) use crate::zengin::npc_actions::{
    AttackAction, GoToNpcAction, GoToWaypointAction, MobUsePose, StandUpAction, UseItemStateAction,
    UseMobAction,
};
pub(crate) use crate::zengin::npc_corpse::NpcCorpseMarker;
pub(crate) use crate::zengin::npc_data::NpcHandle;
pub(crate) use crate::zengin::npc_perception::{NpcAutoTalkOptions, VisibilityRefresh};
pub(crate) use crate::zengin::npc_routines::NpcEquippedWeapon;
pub(crate) use crate::zengin::npc_routines::NpcHealth;
pub(crate) use crate::zengin::npc_routines::NpcWeaponMode;
pub(crate) use crate::zengin::npc_routines::RoutineExecutor;
pub(crate) use crate::zengin::resources::CachedAssetHandles;
pub(crate) use crate::zengin::resources::NPC_BODY;
pub(crate) use crate::zengin::resources::{
    DYNAMIC_OBJECT, SELECTABLE, STATIC_OBJECT, ZenGinModelComponentLoaded,
};
pub(crate) use crate::zengin::script::bridge::ScriptChangesApplied;
pub(crate) use crate::zengin::script::component::ScriptResource;
pub(crate) use crate::zengin::script::journal::{JournalSection, JournalStatus, JournalTopic};
pub(crate) use crate::zengin::script::manager::ItemDisplayData;
pub(crate) use crate::zengin::script::manager::ScriptManager;
pub(crate) use crate::zengin::script::state::ChapterIntro;
pub(crate) use crate::zengin::script::state::ScreenMessage;
pub(crate) use crate::zengin::script::state::ScriptSound;
pub(crate) use crate::zengin::script::state::SetGameTime;
pub(crate) use crate::zengin::script::state::SvmLine;

pub(crate) use crate::zengin::script::state::WeaponClass;
pub(crate) use crate::zengin::script::state::WeaponKind;
pub(crate) use crate::zengin::script::state::WeaponMode;
pub(crate) use crate::zengin::script::state::{ClearAiQueueChange, NpcEntitiesChanged};
pub(crate) use crate::zengin::script::state::{
    ItemRuntime, MobRoutineEntry, NpcRuntime, ScriptMessages, UseMobChange,
};
pub(crate) use crate::zengin::script::string_value::StringCanon;
pub(crate) use crate::zengin::script::symbol_id::InstanceId;
pub(crate) use crate::zengin::script::{
    MENU_FLAG_ALIGN_CENTER, MENU_FLAG_DONT_SCALE_DIM, MENU_FLAG_SHOW_INFO, MENU_ITEM_FLAG_DISABLED,
    MENU_ITEM_FLAG_TXT_CENTER, MENU_SCRIPT_DIV, MenuDefinition, MenuItemDefinition,
    MenuScriptResource, SEL_ACTION_BACK, SEL_ACTION_CLOSE, SEL_ACTION_START_MENU,
    ensure_menu_script,
};
pub(crate) use crate::zengin::statistics::StatisticsZengin;
pub(crate) use crate::zengin::waypoint_graph::WaypointGraph;
pub(crate) use crate::zengin::waypoint_graph::add_waypoint;
pub(crate) use crate::zengin::world_entity::{
    GameChest, GameItem, GameMob, GameNpc, PlayerCharacter, StaticMesh, WorldEntityMarker,
    WorldMesh,
};
pub(crate) use crate::zengin::world_plugin::{
    CurrentWorld, LoadingWorld, NewGame, NpcSpawnNameFilter, RestartWorld, START_WORLD,
    begin_world_load, load_world_prototypes, request_initial_world, request_player_spawn,
    request_world_spawn,
};
pub(crate) use crate::zengin::world_spawn::{
    SpawnFromSavedObject, SpawnPoint, SpawnPositionFilter, VobPrototypes, WorldSpawnMessages,
};

/// Plugin which provides Zengin related functionality
pub struct ZenginPlugin;

impl Plugin for ZenginPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(crate::zengin::audio::ZenginAudioPlugin);
        app.add_plugins(crate::zengin::world_plugin::ZenGinWorldPlugin);
        app.add_plugins(crate::zengin::mobs::MobsPlugin);
        app.add_plugins(crate::zengin::movers::MoversPlugin);
        app.add_plugins(crate::zengin::change_level::ChangeLevelPlugin);
        app.add_plugins(crate::zengin::npc_routines::NpcRoutinesPlugin);
        app.add_plugins(crate::zengin::npc_perception::NpcPerceptionPlugin);
        app.add_plugins(crate::zengin::interaction::selection::SelectionPlugin);
        app.add_plugins(crate::zengin::instance_index::InstanceIndexPlugin);
        app.add_plugins(crate::zengin::script::bridge::ScriptBridgePlugin);
        app.add_plugins(crate::zengin::item_visual::ItemVisualPlugin);
        app.add_plugins(crate::zengin::interaction::PlayerInteractionPlugin);
        app.add_plugins(crate::zengin::interaction::ranged::RangedCombatPlugin);
        app.add_plugins(crate::zengin::statistics::StatisticsZenginPlugin);
        app.add_plugins(crate::zengin::AnimationTriggersPlugin);
        app.add_plugins(crate::zengin::script::component::GothicScriptsPlugin);
        app.add_plugins(crate::zengin::dialog::GameDialogPlugin);
    }
}

/// Fields shared by every `oCMob` vob, whether interactive or a container.
#[derive(Debug, Component, Default, Clone, Reflect)]
pub struct MobBase {
    /// Index of the vob in the ZEN archive, stable across loads.
    pub vob_id: u32,
    /// Plain vob name from the ZEN, used by scripts and trigger targets.
    pub vob_name: String,
    /// In-game focus name (`oCMOB.name`), a script string variable name.
    pub focus_name: String,
}

/// Pose and state data shared by interactive mobs and chests.
#[derive(Debug, Component, Default, Reflect)]
pub struct MobPose {
    /// `AI_UseMob` / player animation scheme derived from the visual stem.
    pub scheme: String,
    /// Visual file stem, the `<STEM>` of the vob's own animation set.
    pub visual_stem: String,
    /// Number of interaction states.
    pub state_num: i32,
    /// `ZS_POS*` attachment nodes, `None` until the model asset is scanned.
    pub positions: Option<Vec<MobPosition>>,
    /// Model-space offset the rendered mesh is shifted by, in world units.
    /// Skinned mobs lose the hierarchy root translation in the joint chain, so
    /// their visible origin sits this far from the vob origin. Pose animation
    /// roots are authored in the unshifted model space and have to be anchored
    /// to the rendered origin instead.
    pub rendered_origin: Vec3,
    /// Door: use from where the player stands instead of snapping to the
    /// attachment point.
    pub no_move: bool,
    /// Current interaction state.
    pub state: i32,
}
