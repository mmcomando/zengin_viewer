//! Command line options of the application.

use std::path::PathBuf;

use bevy::prelude::*;
use clap::Parser;

/// Command line options of the application.
#[derive(Parser, Resource, Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
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
