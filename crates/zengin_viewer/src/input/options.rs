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
        if options.deterministic_time() {
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                SCENARIO_FRAME_DELTA,
            ));
            app.init_resource::<bevy::time::Time<bevy::time::Fixed>>();
            app.world_mut()
                .resource_mut::<bevy::time::Time<bevy::time::Fixed>>()
                .set_timestep(SCENARIO_FRAME_DELTA);
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
    /// Replay the commands from this RON file (any location). Paths inside
    /// the file (screenshots) are resolved relative to
    /// `misc/scenario_output/<file stem>/` and must not be absolute.
    #[arg(long = "replay-actions", value_name = "PATH")]
    replay_actions: Option<PathBuf>,
    /// Start in the main menu instead of starting a new game immediately.
    #[arg(long)]
    main_menu: bool,
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

    pub fn replay_actions(&self) -> Option<&PathBuf> {
        self.replay_actions.as_ref()
    }

    pub fn main_menu(&self) -> bool {
        self.main_menu
    }

    /// Recording or replaying requires deterministic frame timing.
    pub fn deterministic_time(&self) -> bool {
        self.save_actions.is_some() || self.replay_actions.is_some()
    }
}

/// Fixed frame delta used when recording or replaying, so recorded frame
/// numbers map to the same simulated time across runs.
pub const SCENARIO_FRAME_DELTA: std::time::Duration = std::time::Duration::from_millis(20);

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
