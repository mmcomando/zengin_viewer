#![warn(missing_docs)]
//! Application to view world from Gothic 2 game

// #![warn(clippy::pedantic, clippy::restriction)]
#![warn(clippy::pedantic, clippy::restriction)]
#![allow(
    // From clippy::restriction
    clippy::blanket_clippy_restriction_lints, // We decide what to allow, if new ones come then we decide if we want them


    // Maybe reconsider
    clippy::definition_in_module_root,
    // macros debug/info/warn blow up complexity without reason, which
    // might force us to disable cognitive_complexity, for now enable
    // clippy::cognitive_complexity,

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

mod audio;
mod debug;
mod game_state;
mod gameplay;
mod graphics;
mod gui;
mod gui_debug;
mod input;
mod save;
mod test_world;
mod xr;
mod zengin;

/// Directory of the main crate assets, the folder the
/// [`bevy::asset::AssetServer`] reads from. Resolved against the crate
/// directory so it is found regardless of the working directory.
#[must_use]
pub fn main_assets_dir() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets")
}

use crate::audio::SoundPlugin;
use crate::game_state::{AppState, GameStatePlugin};
use crate::gameplay::camera::CameraPlugin;
use crate::gameplay::character::CharacterControllerPlugin;
use crate::gameplay::distance::DistancePlugin;
use crate::gameplay::game_time::GameTimePlugin;
use crate::graphics::{LightingPlugin, RenderOptionsPlugin, SkeletalAnimations, SkyBoxPlugin};
use crate::gui::GameLogPlugin;
use crate::gui_debug::GuiDebugPlugin;
use crate::input::{ActionsPlugin, Options};
use crate::xr::{XrPlugin, is_xr_enabled};
use crate::zengin::SpeechSounds;
use crate::zengin::ZenginPlugin;
use crate::zengin::{ZenginVfs, create_zengin_asset_loader, create_zengin_vfs};
use avian3d::prelude::*;

use bevy::prelude::*;
use bevy::window::PresentMode;

fn main() {
    let mut app = App::new();
    // Command line options, scenario handling has to be registered first.
    app.add_plugins(crate::input::OptionsPlugin);
    // This app custom assert source, has to be registered before default plugins.
    let vfs = create_zengin_vfs();
    app.insert_resource(ZenginVfs(vfs.clone()));
    app.insert_resource(SpeechSounds::new(vfs.clone()));
    app.register_asset_source("zengin", create_zengin_asset_loader(vfs));

    // Window options selection might be quite complicated if we consider XR/render-less/record/replay
    let window_plugin_options = window_plugin_options(&app);
    // XR or standard rendering
    if is_xr_enabled() {
        app.add_plugins(XrPlugin::new(window_plugin_options));
    } else if app.world().resource::<Options>().renderless() {
        crate::input::add_renderless_default_plugins(&mut app, window_plugin_options);
    } else {
        app.add_plugins(
            DefaultPlugins
                .set(window_plugin_options)
                .set(crate::input::scenario_log_plugin()),
        );
    }
    let initial_state = if app.world().resource::<Options>().main_menu() {
        AppState::MainMenu
    } else {
        // Loading starts in its own phase and transitions to `InGame` once the
        // world mesh and its colliders exist.
        AppState::LoadScreen
    };
    app.insert_state(initial_state);
    let scenario_enabled = app.world().resource::<Options>().scenario_enabled();

    // Plugins from crates
    app.add_plugins(PhysicsPlugins::default());

    // Logs a per-frame marker so `debug_perf` output can be grouped by frame.
    #[cfg(feature = "debug_perf")]
    app.add_plugins(dev_util::perf_debug::PerfDebugPlugin);

    // This app custom plugins
    app.add_plugins(GameStatePlugin)
        .add_plugins(ActionsPlugin)
        // World and its content
        .add_plugins(ZenginPlugin)
        .add_plugins(crate::save::QuickSavePlugin)
        // Environment
        .add_plugins(SkyBoxPlugin)
        .add_plugins(LightingPlugin)
        // Gameplay
        .add_plugins(CharacterControllerPlugin)
        .add_plugins(CameraPlugin)
        .add_plugins(DistancePlugin)
        .add_plugins(GameTimePlugin)
        .add_plugins(SkeletalAnimations)
        .add_plugins(crate::test_world::TestWorldPlugin)
        .add_plugins(dev_util::updater::plugin::AmortizedExecutorPlugin {
            unlimited: scenario_enabled,
            active_state: AppState::InGame,
        })
        // Presentation and UI
        .add_plugins(RenderOptionsPlugin)
        .add_plugins(SoundPlugin)
        .add_plugins(GameLogPlugin)
        .add_plugins(crate::gui::ChapterScreenPlugin)
        .add_plugins(crate::gui::HealthBarsPlugin)
        .add_plugins(crate::gui::InventoryUiPlugin)
        .add_plugins(crate::gui::JournalUiPlugin)
        .add_plugins(crate::gui::LoadingScreenPlugin)
        .add_plugins(crate::gui::MainMenuPlugin)
        .add_plugins(crate::gui::ScreenMessagesPlugin)
        .add_plugins(crate::gui::StatsUiPlugin);

    // Debug-only observation systems; they consume game messages and must be
    // added after the plugins that register them.
    app.add_plugins(crate::debug::DebugPlugin);

    // The debug UI and gizmos are disabled with `--no-debug-gui`, GuiDebugPlugin
    // handles that.
    app.add_plugins(GuiDebugPlugin);

    // The default world spawn is explicit: a direct start requests it here, a
    // main menu run requests it when New Game is picked, and a scenario run
    // requests its worlds itself.
    if initial_state == AppState::LoadScreen && !scenario_enabled {
        crate::zengin::request_initial_world(app.world_mut());
    }

    // Run
    app.run();
}

fn window_plugin_options(app: &App) -> WindowPlugin {
    let mut primary_window = Window {
        title: "ZenGin Walker".into(),
        present_mode: PresentMode::AutoNoVsync,
        // present_mode: PresentMode::AutoVsync,
        ..default()
    };
    let scenario_enabled = app.world().resource::<Options>().scenario_enabled();
    crate::input::apply_scenario_window_settings(&mut primary_window, scenario_enabled);
    WindowPlugin {
        primary_window: Some(primary_window),
        ..default()
    }
}
