use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use bevy::prelude::*;

use crate::input::record::RecordedAction;

use super::scenario::ScheduledCommand;

/// Replay schedule loaded from the scenario file plus the replay state.
#[derive(Resource)]
pub(super) struct ActionReplay {
    /// Workdir used to resolve relative paths inside the commands.
    pub(super) workdir: PathBuf,
    /// Root directory of the `TestScreenshot` reference screenshots.
    pub(super) reference_root: PathBuf,
    /// Copy produced test screenshots into the reference instead of comparing.
    pub(super) update_reference: bool,
    /// Screenshots need rendering, so they are skipped in renderless runs.
    pub(super) renderless: bool,
    /// Commands with resolved frames, sorted by frame.
    pub(super) commands: Vec<ScheduledCommand>,
    /// Index of the next event to apply.
    pub(super) next: usize,
    pub(super) frame: u64,
    /// `ExitGame` was requested but the test run still has to finish capturing
    /// and comparing screenshots before the app can exit.
    pub(super) exit_requested: bool,
    /// Buttons currently held by the replay.
    pub(super) held_buttons: HashSet<RecordedAction>,
    /// Buttons whose press event was processed this frame; these get a real
    /// `JustPressed` press, later held frames only keep the pressed state.
    pub(super) started_buttons: HashSet<RecordedAction>,
    /// Buttons released again right after their press frame, so a
    /// `PressRelease` stays held for exactly one frame.
    pub(super) released_buttons: HashSet<RecordedAction>,
    /// Latest dual axis values applied by the replay.
    pub(super) held_axes: HashMap<RecordedAction, Vec2>,
}

/// Teleport and camera location commands of the current frame, buffered by
/// [`replay_actions`](super::actions::replay_actions) and applied by
/// [`apply_replay_locations`](super::command::apply_replay_locations).
#[derive(Resource, Default)]
pub(super) struct PendingLocations {
    pub(super) player: Vec<(Vec3, Vec3)>,
    pub(super) camera: Vec<(Vec3, Vec3)>,
}
