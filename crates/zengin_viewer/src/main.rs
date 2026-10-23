#![warn(missing_docs)]
//! Application to view world from Gothic 2 game

// #![warn(clippy::pedantic, clippy::restriction)]
#![warn(clippy::pedantic, clippy::restriction)]
#![allow(
    // From clippy::restriction
    clippy::blanket_clippy_restriction_lints, // We decide what to allow, if new ones come then we decide if we want them
    clippy::single_call_fn,
    clippy::implicit_return,
    clippy::absolute_paths,
    clippy::missing_docs_in_private_items,
    clippy::clone_on_ref_ptr,
    clippy::doc_paragraphs_missing_punctuation,
    clippy::missing_trait_methods,
    clippy::missing_inline_in_public_items,
    clippy::shadow_reuse,
    clippy::str_to_string,
    clippy::arbitrary_source_item_ordering,
    clippy::pattern_type_mismatch,
    clippy::float_arithmetic,
    clippy::arithmetic_side_effects,
    clippy::question_mark_used,
    clippy::expect_used,
    clippy::as_conversions,
    clippy::iter_over_hash_type,
    clippy::print_stdout,
    clippy::default_numeric_fallback,
    clippy::std_instead_of_core,
    clippy::integer_division_remainder_used,
    clippy::integer_division,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::missing_assert_message,
    clippy::unwrap_used,
    clippy::module_name_repetitions,
    clippy::unused_trait_names,
    clippy::impl_trait_in_params,
    clippy::if_then_some_else_none,
    clippy::use_debug,
    clippy::renamed_function_params,
    clippy::pub_with_shorthand,
    clippy::mod_module_files,
    clippy::std_instead_of_alloc,
    clippy::string_add,
    clippy::missing_asserts_for_indexing,
    clippy::semicolon_outside_block,
    clippy::empty_structs_with_brackets,
    clippy::rest_pattern_accessible_field,
    clippy::unimplemented,
    clippy::modulo_arithmetic,
    clippy::let_underscore_untyped,
    clippy::unreachable,
    clippy::map_with_unused_argument_over_ranges,
    clippy::little_endian_bytes,
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    clippy::inline_trait_bounds,
    clippy::partial_pub_fields,
    clippy::single_char_lifetime_names,
    clippy::else_if_without_else,
    clippy::field_scoped_visibility_modifiers,
    clippy::unseparated_literal_suffix,
    clippy::shadow_unrelated,


    // maybe reconsider
    clippy::definition_in_module_root,

    // macros debug/info/warn blow up complexity without reason, which
    // might force us to disable cognitive_complexity, for now enable
    // clippy::cognitive_complexity,

    // other clippy
    clippy::unused_async_trait_impl,
    clippy::redundant_closure_for_method_calls,
    clippy::trivially_copy_pass_by_ref,
    clippy::assert_is_empty,
    clippy::cast_precision_loss,
    clippy::needless_pass_by_value,
    clippy::cast_possible_truncation,
    clippy::uninlined_format_args,
    clippy::needless_return,
    clippy::enum_variant_names,
    clippy::too_many_lines,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::manual_assert,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::too_many_arguments,
    clippy::wildcard_imports
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
        .add_plugins(crate::gui::InventoryUiPlugin);

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
