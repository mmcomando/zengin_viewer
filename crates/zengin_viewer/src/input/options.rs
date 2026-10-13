use bevy::prelude::*;
use clap::Parser;

use crate::input::scenario::{Scenario, ScenarioPlugin};

/// Command line options of the application.
#[derive(Parser, Resource, Debug)]
pub struct Options {
    /// Run a predefined scenario instead of the normal game loop.
    #[arg(long)]
    run_scenario: Option<Scenario>,
}

impl Options {
    pub fn scenario(&self) -> Scenario {
        self.run_scenario.unwrap_or(Scenario::Normal)
    }
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
