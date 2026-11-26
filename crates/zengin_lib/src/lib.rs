//! Zengin world formats, gameplay, GUI and the script host.
//!
//! The viewer crate provides the application shell (window, assets, input
//! devices) on top of these systems.

#![warn(clippy::pedantic, clippy::restriction)]
#![allow(
    // From clippy::restriction
    clippy::blanket_clippy_restriction_lints, // We decide what to allow, if new ones come then we decide if we want them

    // API-shape lints are deferred for now.
    clippy::exhaustive_enums,
    clippy::exhaustive_structs,
    clippy::must_use_candidate,
    clippy::return_self_not_must_use,
    clippy::pub_use,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::implicit_hasher,
    clippy::new_without_default,
    clippy::should_implement_trait,

    // Maybe reconsider
    clippy::definition_in_module_root,

    // Maybe do in future to make app more stable
    clippy::as_conversions,
    clippy::expect_used,
    clippy::missing_assert_message,
    clippy::missing_asserts_for_indexing,
    clippy::panic,
    clippy::print_stdout,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::unwrap_used,
    clippy::use_debug,

    // Math
    clippy::arithmetic_side_effects,
    clippy::default_numeric_fallback,
    clippy::float_arithmetic,
    clippy::integer_division,
    clippy::integer_division_remainder_used,
    clippy::little_endian_bytes,
    clippy::modulo_arithmetic,

    // Style chosen for this project
    clippy::absolute_paths,
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    clippy::arbitrary_source_item_ordering,
    clippy::clone_on_ref_ptr,
    clippy::doc_paragraphs_missing_punctuation,
    clippy::else_if_without_else,
    clippy::empty_structs_with_brackets,
    clippy::field_scoped_visibility_modifiers,
    clippy::if_then_some_else_none,
    clippy::impl_trait_in_params,
    clippy::implicit_return,
    clippy::indexing_slicing,
    clippy::inline_trait_bounds,
    clippy::iter_over_hash_type,
    clippy::let_underscore_untyped,
    clippy::map_with_unused_argument_over_ranges,
    clippy::missing_docs_in_private_items,
    clippy::missing_inline_in_public_items,
    clippy::missing_trait_methods,
    clippy::mod_module_files,
    clippy::module_name_repetitions,
    clippy::partial_pub_fields,
    clippy::pattern_type_mismatch,
    clippy::pub_with_shorthand,
    clippy::question_mark_used,
    clippy::rest_pattern_accessible_field,
    clippy::semicolon_outside_block,
    clippy::shadow_reuse,
    clippy::shadow_unrelated,
    clippy::single_call_fn,
    clippy::single_char_lifetime_names,
    clippy::std_instead_of_alloc,
    clippy::std_instead_of_core,
    clippy::str_to_string,
    clippy::string_add,
    clippy::unseparated_literal_suffix,
    clippy::unused_trait_names,

    // Non clippy::restriction lints
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,

    // Style/simplicity choice
    clippy::assert_is_empty,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::redundant_closure_for_method_calls,
    clippy::trivially_copy_pass_by_ref,
    clippy::unused_async_trait_impl,

    // Bevy specific
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::wildcard_imports,

    // To reconsider
    clippy::manual_assert,
    clippy::needless_return,
    clippy::too_many_lines,
    clippy::uninlined_format_args
)]

mod animation;
mod audio;
mod change_level;
mod common;
mod dialog;
mod fight_ai;
mod game_state;
mod gameplay;
mod gui;
mod instance_index;
mod interaction;
mod item_data;
mod item_visual;
mod loaders;
mod mobs;
mod movers;
mod npc_actions;
mod npc_corpse;
mod npc_data;
mod npc_perception;
mod npc_routines;
mod objects_to_entities;
mod resources;
mod save;
mod script;
mod statistics;
mod visual;
mod waypoint_graph;
mod world_entity;
mod world_plugin;
mod world_spawn;

use bevy::prelude::*;

// Public API consumed by the viewer crate.
pub use crate::animation::{AnimatedJoint, AnimationController, SkeletalAnimations};
pub use crate::audio::{PlaySound, PlayVoice, StopVoice};
pub use crate::dialog::DialogRequest;
pub use crate::dialog::DialogUiState;
pub use crate::dialog::DialogVoice;
pub use crate::dialog::SpeechSounds;
pub use crate::game_state::{AppState, GameStatePlugin};
pub use crate::interaction::selection::Selection;
pub use crate::interaction::{NpcDied, grant_test_items, seed_test_journal};
pub use crate::loaders::vdf_reader::{ZenginVfs, create_zengin_asset_loader, create_zengin_vfs};
pub use crate::mobs::NpcMobUse;
pub use crate::movers::GameMover;
pub use crate::npc_actions::NpcActionQueue;
pub use crate::npc_actions::enqueue_npc_goto;
pub use crate::npc_actions::{
    AttackAction, GoToNpcAction, GoToWaypointAction, MobUsePose, StandUpAction, UseItemStateAction,
    UseMobAction,
};
pub use crate::npc_perception::NpcAutoTalkOptions;
pub use crate::npc_routines::NpcEquippedWeapon;
pub use crate::npc_routines::NpcHealth;
pub use crate::npc_routines::NpcOverlay;
pub use crate::npc_routines::NpcWeaponMode;
pub use crate::npc_routines::RoutineExecutor;
pub use crate::resources::{DYNAMIC_OBJECT, STATIC_OBJECT, ZenGinModelComponentLoaded};
pub use crate::script::component::ScriptResource;
pub use crate::script::manager::ScriptManager;
pub use crate::script::state::ScriptSound;
pub use crate::script::state::SetGameTime;
pub use crate::script::state::WeaponKind;
pub use crate::script::state::WeaponMode;
pub use crate::script::state::{ClearAiQueueChange, NpcEntitiesChanged};
pub use crate::script::state::{ScriptMessages, UseMobChange};
pub use crate::script::string_value::StringCanon;
pub use crate::script::symbol_id::InstanceId;
pub use crate::statistics::StatisticsZengin;
pub use crate::waypoint_graph::WaypointGraph;
pub use crate::waypoint_graph::add_waypoint;
pub use crate::world_entity::{
    GameChest, GameItem, GameMob, GameNpc, PlayerCharacter, StaticMesh, WorldEntityMarker,
    WorldMesh,
};
pub use crate::world_plugin::{
    NpcSpawnNameFilter, RestartWorld, START_WORLD, load_world_prototypes, request_initial_world,
    request_player_spawn, request_world_spawn,
};
pub use crate::world_spawn::{
    SpawnFromSavedObject, SpawnPoint, SpawnPositionFilter, VobPrototypes, WorldSpawnMessages,
};

pub use crate::gameplay::camera::{
    CameraPlugin, CameraResource, EditorMode, TELEPORT_LOCATIONS, TeleportToLocation,
};
pub use crate::gameplay::character::CharacterControllerPlugin;
pub use crate::gameplay::character_camera::camera_toggle_input;
pub use crate::gameplay::distance::DistancePlugin;
pub use crate::gameplay::game_time::{GameTime, GameTimePlugin};
pub use crate::gameplay::objects::{NpcBodyRootOffset, NpcModel};
pub use crate::gui::{
    ChapterScreenPlugin, ChapterScreenState, GameLog, GameLogPlugin, HealthBarsPlugin,
    InventoryUiPlugin, InventoryUiState, JournalUiPlugin, JournalUiState, LoadingScreenPlugin,
    LockpickOpened, LootSource, MainMenuPlugin, OpenLockpick, OpenLoot, ScreenMessagesPlugin,
    StatsUiPlugin, StatsUiState,
};
pub use crate::save::format::{SavedObject, SavedObjectBuilder};
pub use crate::save::{QuickLoad, QuickSave, QuickSavePlugin};

// Crate-internal reexports kept for `crate::<Name>` convenience paths.
pub(crate) use crate::animation::{
    AnimationData, AnimationEnds, AnimationGraph, AnimationSample, AnimationState,
    AnimationTriggers, AnimationTriggersPlugin, AutoAnimSequencer, AutoAnimSequencerSet,
    BoneAnimationData, DesiredAnimation, FrameState, MeleeAttackKind, NpcAnimations,
    NpcHitReaction, PlayerMeleeCombo, SetDesiredAnimation, animation_name_from_path,
    is_melee_attack_animation, npc_hit_reaction_end, play_hit_reaction,
};
pub(crate) use crate::audio::PendingMusic;
pub(crate) use crate::common::MobPosition;
pub(crate) use crate::common::MusicZones;
pub(crate) use crate::common::animation_file_name;
pub(crate) use crate::common::choose_animation_scheme;
pub(crate) use crate::common::face_direction_quat;
pub(crate) use crate::common::get_animation_path;
pub(crate) use crate::common::get_dialog_sound_path;
pub(crate) use crate::common::get_full_texture_path;
pub(crate) use crate::common::gothic_dat_path;
pub(crate) use crate::common::gothic2_dir;
pub(crate) use crate::common::to_world_units;
pub(crate) use crate::fight_ai::{FightAiResource, load_fight_ai_tables};
pub(crate) use crate::game_state::{LoadingScreenReady, loading_screen_ready, world_is_loaded};
pub(crate) use crate::interaction::MeleeHit;
pub(crate) use crate::interaction::ranged::ArrowShot;
pub(crate) use crate::interaction::selection::SelectionKind;
pub(crate) use crate::item_data::ItemHandle;
pub(crate) use crate::loaders::vdf_reader::get_zengin_asset_bytes;
pub(crate) use crate::loaders::vfs::{resolve_sound_asset, resolve_texture_asset};
pub(crate) use crate::mobs::{MobHiddenModel, MobOccupant, MobRegistry};
pub(crate) use crate::movers::VobTrigger;
pub(crate) use crate::npc_corpse::NpcCorpseMarker;
pub(crate) use crate::npc_data::NpcHandle;
pub(crate) use crate::npc_perception::VisibilityRefresh;
pub(crate) use crate::npc_routines::{AnimationFiles, NpcTalentOverlay, talent_overlay_for};
pub(crate) use crate::resources::CachedAssetHandles;
pub(crate) use crate::resources::NPC_BODY;
pub(crate) use crate::resources::SELECTABLE;
pub(crate) use crate::script::bridge::ScriptChangesApplied;
pub(crate) use crate::script::journal::{JournalSection, JournalStatus, JournalTopic};
pub(crate) use crate::script::manager::ItemDisplayData;
pub(crate) use crate::script::state::ChapterIntro;
pub(crate) use crate::script::state::ScreenMessage;
pub(crate) use crate::script::state::SvmLine;
pub(crate) use crate::script::state::WeaponClass;
pub(crate) use crate::script::state::{ItemRuntime, MobRoutineEntry, NpcRuntime};
pub(crate) use crate::script::{
    MENU_FLAG_ALIGN_CENTER, MENU_FLAG_DONT_SCALE_DIM, MENU_FLAG_SHOW_INFO, MENU_ITEM_FLAG_DISABLED,
    MENU_ITEM_FLAG_TXT_CENTER, MENU_SCRIPT_DIV, MenuDefinition, MenuItemDefinition,
    MenuScriptResource, SEL_ACTION_BACK, SEL_ACTION_CLOSE, SEL_ACTION_START_MENU,
    ensure_menu_script,
};
pub(crate) use crate::world_plugin::{CurrentWorld, LoadingWorld, NewGame, begin_world_load};

/// Plugin which provides Zengin related functionality
pub struct ZenginPlugin;

impl Plugin for ZenginPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(crate::audio::ZenginAudioPlugin);
        app.add_plugins(crate::world_plugin::ZenGinWorldPlugin);
        app.add_plugins(crate::mobs::MobsPlugin);
        app.add_plugins(crate::movers::MoversPlugin);
        app.add_plugins(crate::change_level::ChangeLevelPlugin);
        app.add_plugins(crate::npc_routines::NpcRoutinesPlugin);
        app.add_plugins(crate::npc_perception::NpcPerceptionPlugin);
        app.add_plugins(crate::interaction::selection::SelectionPlugin);
        app.add_plugins(crate::instance_index::InstanceIndexPlugin);
        app.add_plugins(crate::script::bridge::ScriptBridgePlugin);
        app.add_plugins(crate::item_visual::ItemVisualPlugin);
        app.add_plugins(crate::interaction::PlayerInteractionPlugin);
        app.add_plugins(crate::interaction::ranged::RangedCombatPlugin);
        app.add_plugins(crate::statistics::StatisticsZenginPlugin);
        app.add_plugins(crate::AnimationTriggersPlugin);
        app.add_plugins(crate::script::component::GothicScriptsPlugin);
        app.add_plugins(crate::dialog::GameDialogPlugin);
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
