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
mod gameplay;
mod graphics;
mod gui;
mod gui_debug;
mod input;
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
use crate::gameplay::camera::CameraPlugin;
use crate::gameplay::character::CharacterControllerPlugin;
use crate::gameplay::distance::DistancePlugin;
use crate::gameplay::game_time::GameTimePlugin;
use crate::graphics::{
    DiscoLamp, LightingPlugin, RenderOptionsPlugin, SkeletalAnimations, SkyBoxPlugin,
};
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
        // Default plugin
        app.add_plugins(DefaultPlugins.set(window_plugin_options));
    }

    // Plugins from crates
    app.add_plugins(PhysicsPlugins::default());

    // This app custom plugins
    app.add_plugins(ActionsPlugin)
        // World and its content
        .add_plugins(ZenginPlugin)
        // Environment
        .add_plugins(SkyBoxPlugin)
        .add_plugins(LightingPlugin)
        .add_plugins(DiscoLamp)
        // Gameplay
        .add_plugins(CharacterControllerPlugin)
        .add_plugins(CameraPlugin)
        .add_plugins(DistancePlugin)
        .add_plugins(GameTimePlugin)
        .add_plugins(SkeletalAnimations)
        // Presentation and UI
        .add_plugins(RenderOptionsPlugin)
        .add_plugins(SoundPlugin)
        .add_plugins(GameLogPlugin)
        .add_plugins(crate::gui::HealthBarsPlugin)
        .add_plugins(crate::gui::InventoryUiPlugin)
        .add_plugins(crate::gui::ScreenMessagesPlugin);

    // The egui debug UI and gizmos are skipped for scenario replays so
    // screenshots contain no debug overlays, GuiDebugPlugin handles that.
    app.add_plugins(GuiDebugPlugin);

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
    let replay_enabled = app.world().resource::<Options>().replay_actions().is_some();
    crate::input::apply_scenario_window_settings(&mut primary_window, replay_enabled);
    WindowPlugin {
        primary_window: Some(primary_window),
        ..default()
    }
}
