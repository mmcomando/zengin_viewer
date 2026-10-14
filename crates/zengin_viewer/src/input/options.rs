use bevy::prelude::*;
use clap::Parser;

use crate::input::scenario::{Scenario, ScenarioPlugin};

/// Command line options of the application.
#[derive(Parser, Resource, Debug)]
pub struct Options {
    /// Run a predefined scenario instead of the normal game loop.
    #[arg(long)]
    run_scenario: Option<Scenario>,
    /// Run without rendering, only application logic and logs.
    #[arg(long)]
    renderless: bool,
    /// Stop the application after this many frames (mainly for headless runs).
    #[arg(long)]
    frames: Option<u64>,
}

impl Options {
    pub fn scenario(&self) -> Scenario {
        self.run_scenario.unwrap_or(Scenario::Normal)
    }

    pub fn renderless(&self) -> bool {
        self.renderless
    }

    pub fn frames(&self) -> Option<u64> {
        self.frames
    }
}

/// Registers the Shader asset type in headless mode, normally done by RenderPlugin.
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

pub struct OptionsPlugin;

impl Plugin for OptionsPlugin {
    fn build(&self, app: &mut App) {
        let options = Options::parse();
        info!("Command line options: {options:?}");
        app.add_plugins(ScenarioPlugin::new(options.scenario()));
        app.insert_resource(options);
    }
}
