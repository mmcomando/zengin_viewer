use std::path::PathBuf;

use bevy::prelude::*;
use clap::Parser;

pub struct OptionsPlugin;

impl Plugin for OptionsPlugin {
    fn build(&self, app: &mut App) {
        let options = Options::parse();
        info!("Command line options: {options:?}");
        app.insert_resource(options.clone());
        if let Some(frames) = options.frames() {
            app.add_plugins(FrameLimitPlugin(frames));
        }
        // Recording or replaying runs with a fixed frame delta, so frame numbers
        // map to the same simulated time and physics steps in every run.
        if options.fixed_time() {
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                FIXED_FRAME_DELTA,
            ));
            app.init_resource::<bevy::time::Time<bevy::time::Fixed>>();
            app.world_mut()
                .resource_mut::<bevy::time::Time<bevy::time::Fixed>>()
                .set_timestep(FIXED_FRAME_DELTA);
        }
    }
}

/// Registers the Shader asset type in headless mode, normally done by `RenderPlugin`.
pub struct RegisterShaderAsset;

impl Plugin for RegisterShaderAsset {
    fn build(&self, app: &mut App) {
        app.init_asset::<bevy::shader::Shader>();
        app.init_asset_loader::<bevy::shader::ShaderLoader>();
    }
}

/// Exits the application after running a fixed number of Update frames.
pub struct FrameLimitPlugin(pub u64);

impl Plugin for FrameLimitPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(FrameLimit(self.0));
        app.add_systems(Update, frame_limit);
    }
}

/// Command line options of the application.
#[derive(Parser, Resource, Debug, Clone)]
pub struct Options {
    /// Run without rendering, only application logic and logs.
    #[arg(long)]
    renderless: bool,
    /// Stop the application after this many frames (mainly for headless runs).
    #[arg(long)]
    frames: Option<u64>,
    /// Record every triggered action with its frame number into this file.
    /// An existing file is removed at startup so recording starts clean.
    #[arg(long = "save-actions", value_name = "PATH")]
    save_actions: Option<PathBuf>,
    /// Run the scenario with this ID. The ID is a path under `misc/scenario`,
    /// with or without the `.ron` extension (e.g. `tests/dialog/lares`).
    /// Screenshots go to `misc/scenario_output/<id>/`, `TestScreenshot`
    /// references live in `misc/scenario_test_references/<id>/`.
    #[arg(long = "scenario", value_name = "ID")]
    scenario: Option<PathBuf>,
    /// Copy the screenshots produced by `TestScreenshot` into the reference
    /// directory instead of comparing them.
    #[arg(long = "update-reference", requires = "scenario")]
    update_reference: bool,
    /// Start in the main menu instead of starting a new game immediately.
    #[arg(long)]
    main_menu: bool,
    /// Use a fixed frame delta instead of the real clock.
    #[arg(long)]
    fixed_time: bool,
    /// Disable the debug GUI (egui window, gizmos and FPS overlay).
    #[arg(long)]
    no_debug_gui: bool,
}

impl Options {
    pub fn renderless(&self) -> bool {
        self.renderless
    }

    pub fn frames(&self) -> Option<u64> {
        self.frames
    }

    pub fn save_actions(&self) -> Option<&PathBuf> {
        self.save_actions.as_ref()
    }

    pub fn scenario(&self) -> Option<&PathBuf> {
        self.scenario.as_ref()
    }

    pub fn update_reference(&self) -> bool {
        self.update_reference
    }

    /// Whether the run replays a scenario. Scenario runs use a hidden window
    /// with a fixed resolution.
    pub fn scenario_enabled(&self) -> bool {
        self.scenario.is_some()
    }

    pub fn main_menu(&self) -> bool {
        self.main_menu
    }

    pub fn fixed_time(&self) -> bool {
        self.fixed_time
    }

    pub fn no_debug_gui(&self) -> bool {
        self.no_debug_gui
    }
}

/// Fixed frame delta used with `--fixed-time`, so frame numbers map to the same
/// simulated time across runs.
pub const FIXED_FRAME_DELTA: std::time::Duration = std::time::Duration::from_millis(20);

#[derive(Resource)]
struct FrameLimit(u64);

fn frame_limit(mut frames: Option<ResMut<FrameLimit>>, mut exit: MessageWriter<AppExit>) {
    let Some(frames) = frames.as_deref_mut() else {
        return;
    };
    if frames.0 > 1 {
        frames.0 -= 1;
        return;
    }
    info!("Frame limit reached, exiting.");
    exit.write(AppExit::Success);
}

/// Adds Bevy default plugins configured for a renderless run: no window, no
/// renderer, only application logic and logs.
pub fn add_renderless_default_plugins(app: &mut App, window_plugin_options: WindowPlugin) {
    // RenderPlugin normally registers the Shader asset, plugins later in the group
    // create Shader handles during build, so it has to be registered after AssetPlugin
    // (needs AssetServer) but before them.
    app.add_plugins(
        DefaultPlugins
            .set(window_plugin_options)
            .set(crate::input::scenario_log_plugin())
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
}
