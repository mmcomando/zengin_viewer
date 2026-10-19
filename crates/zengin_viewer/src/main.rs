#![allow(
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

use crate::audio::sound_manager::GameAudioPlugin;
use crate::gameplay::camera::{CameraPlugin, get_overlay_plugin};
use crate::gameplay::character::CharacterControllerPlugin;
use crate::gameplay::distance::DistancePlugin;
use crate::gameplay::game_time::GameTimePlugin;
use crate::graphics::animation::SkeletalAnimations;
use crate::graphics::disco_lamp::DiscoLamp;
use crate::graphics::lighting::LightingPlugin;
use crate::graphics::render_options::RenderOptionsPlugin;
use crate::graphics::skybox::SkyBoxPlugin;
use crate::gui::game_log::GameLogPlugin;
use crate::gui_debug::egui::GameDebugUI;
use crate::gui_debug::statistics::StatisticsPlugin;
use crate::gui_debug::toggle_visibility::ToggleVisibility;
use crate::input::actions::ActionsPlugin;
use crate::input::options::{Options, RegisterShaderAsset};
use crate::xr::{XrPlugin, is_xr_enabled};
use crate::zengin::SpeechSounds;
use crate::zengin::ZenginPlugin;
use crate::zengin::{
    WaypointDebugPlugin, ZenginVfs, create_zengin_asset_loader, create_zengin_vfs,
};
use avian3d::prelude::*;

use bevy::prelude::*;
use bevy::window::PresentMode;

fn main() {
    let mut app = App::new();
    // Command line options, scenario handling has to be registered first.
    app.add_plugins(crate::input::options::OptionsPlugin);
    // This app custom assert source, has to be registered before default plugins.
    let vfs = create_zengin_vfs();
    app.insert_resource(ZenginVfs(vfs.clone()));
    app.insert_resource(SpeechSounds::new(vfs.clone()));
    app.register_asset_source("zengin", create_zengin_asset_loader(vfs));
    // Default plugin
    let mut primary_window = Window {
        title: "ZenGin Walker".into(),
        present_mode: PresentMode::AutoNoVsync,
        // present_mode: PresentMode::AutoVsync,
        ..default()
    };
    let replay_enabled = app.world().resource::<Options>().replay_actions().is_some();
    crate::input::replay::apply_scenario_window_settings(&mut primary_window, replay_enabled);
    if replay_enabled {
        // Repeatable replays: fixed game time, no progression. GameTimePlugin
        // keeps this resource because init_resource does not overwrite it.
        app.insert_resource(crate::gameplay::game_time::GameTime {
            total_hours: 12.0,
            speed: 0.0,
        });
    }
    let window_plugin_options = WindowPlugin {
        primary_window: Some(primary_window),
        ..default()
    };
    // XR or standard rendering
    if is_xr_enabled() {
        app.add_plugins(XrPlugin::new(window_plugin_options));
    } else if app.world().resource::<Options>().renderless() {
        // Renderless run: no window, no renderer, only application logic and logs.
        // RenderPlugin normally registers the Shader asset, plugins later in the group
        // create Shader handles during build, so it has to be registered after AssetPlugin
        // (needs AssetServer) but before them.
        app.add_plugins(
            DefaultPlugins
                .set(window_plugin_options)
                .disable::<bevy::winit::WinitPlugin>()
                .disable::<bevy::picking::input::PointerInputPlugin>()
                .disable::<bevy::window::WindowPlugin>()
                .disable::<bevy::render::RenderPlugin>()
                .disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>()
                .add_after::<bevy::asset::AssetPlugin>(RegisterShaderAsset),
        );
        // WindowPlugin registers the window-related messages, without it message readers fail.
        app.add_message::<bevy::window::WindowEvent>();
        app.add_message::<bevy::window::FileDragAndDrop>();
        app.add_message::<bevy::window::CursorMoved>();
        app.add_message::<bevy::window::Ime>();
        // WinitPlugin registers the RequestRedraw message.
        app.add_message::<bevy::window::RequestRedraw>();
        app.add_plugins(bevy::app::ScheduleRunnerPlugin::run_loop(
            core::time::Duration::from_secs_f64(1.0 / 60.0),
        ));
    } else {
        app.add_plugins(DefaultPlugins.set(window_plugin_options));
    }
    if let Some(frames) = app.world().resource::<Options>().frames() {
        app.add_plugins(crate::input::options::FrameLimitPlugin(frames));
    }
    // Recording or replaying runs with a fixed frame delta, so frame numbers
    // map to the same simulated time and physics steps in every run.
    if app.world().resource::<Options>().deterministic_time() {
        use crate::input::options::SCENARIO_FRAME_DELTA;
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            SCENARIO_FRAME_DELTA,
        ));
        app.world_mut()
            .resource_mut::<bevy::time::Time<bevy::time::Fixed>>()
            .set_timestep(SCENARIO_FRAME_DELTA);
    }

    // Plugins from crates
    app.add_plugins(PhysicsPlugins::default());
    // FPS overlay requires rendering, skip it in renderless mode.
    if !app.world().resource::<Options>().renderless() {
        if !replay_enabled {
            // Scenario replays must not show debug overlays in screenshots.
            app.add_plugins(get_overlay_plugin());
            app.add_plugins(WaypointDebugPlugin);
        }
    }

    // This app custom plugins
    app.add_plugins(ActionsPlugin)
        .add_plugins(ZenginPlugin)
        .add_plugins(SkyBoxPlugin)
        .add_plugins(ToggleVisibility)
        .add_plugins(CharacterControllerPlugin)
        .add_plugins(DiscoLamp)
        .add_plugins(DistancePlugin)
        .add_plugins(GameTimePlugin)
        .add_plugins(RenderOptionsPlugin)
        .add_plugins(LightingPlugin);
    // The egui debug UI (Info window, camera overlay) is skipped for scenario
    // replays so screenshots contain no debug overlays.
    if !replay_enabled {
        app.add_plugins(GameDebugUI);
    }
    app.add_plugins(StatisticsPlugin)
        .add_plugins(GameAudioPlugin)
        .add_plugins(GameLogPlugin)
        .add_plugins(crate::gui::health_bars::HealthBarsPlugin)
        .add_plugins(crate::gui::inventory::InventoryUiPlugin)
        .add_plugins(CameraPlugin)
        .add_plugins(SkeletalAnimations);

    // Run
    app.run();
}
