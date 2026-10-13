use crate::audio::sound_manager::{PlaySound, SoundManager};
use crate::zengin::common::get_dialog_sound_path;
use crate::zengin::script::component::{DialogTexts, ScriptResource};
use crate::zengin::script::main::InstanceId;
use crate::zengin::script::state::{DialogLine, DialogRoot};
use crate::zengin::sounds::SpeechSounds;
use bevy::log::warn;
use bevy::prelude::*;
use std::time::Duration;

/// How long a single dialog line is displayed during playback
const LINE_DISPLAY_SECONDS: f32 = 1.0;

/// Extra pause after a line's sound finished before the next line is shown
const LINE_GAP_SECONDS: f32 = 0.2;

/// What is currently presented by the dialog UI
#[derive(Default)]
enum DialogUiMode {
    /// Root options of the npc, shown at the bottom center
    #[default]
    Options,
    /// Lines produced by the last executed option, shown one by one at the top
    Lines {
        lines: Vec<DialogLineView>,
        /// Index of the currently displayed line
        index: usize,
        /// Display timer of the current line
        timer: Timer,
    },
}

/// Dialog line prepared for display, with the real text resolved
pub(super) struct DialogLineView {
    pub(super) line: DialogLine,
    /// Real line text, None if the block was not found
    pub(super) text: Option<String>,
    /// Duration of the line's sound file, None if not found/parseable
    pub(super) sound_duration: Option<f32>,
}

/// State driving the dialog UI, it is shown only while `npc_entity` is present
#[derive(Resource)]
pub(super) struct DialogUiState {
    /// Shared handle of the script vm, locked whenever the dialog runs script
    /// functions
    script: ScriptResource,
    pub(super) npc_entity: Option<Entity>,
    pub(super) npc_player: Option<InstanceId>,
    /// Script instance of the npc the dialog is open with
    pub(super) npc_talk_to: Option<InstanceId>,
    pub(super) dialog_roots: Vec<DialogRoot>,
    /// Sub dialog choices populated by the last executed option, shown instead
    /// of `dialog_roots` when not empty
    pub(super) sub_choices: Vec<DialogRoot>,
    /// Highlighted entry in options mode
    pub(super) selected_index: usize,
    mode: DialogUiMode,
    /// Symbol name of the npc the dialog is open with, resolved when the dialog opens
    pub(super) npc_name: String,
    /// Set when `dialog_roots` changed and the option rows have to be re-synced
    pub(super) rows_dirty: bool,
}

impl FromWorld for DialogUiState {
    /// Sets up the state from the script resource of the world
    fn from_world(world: &mut World) -> Self {
        Self {
            script: world.resource::<ScriptResource>().clone(),
            npc_entity: None,
            npc_player: None,
            npc_talk_to: None,
            dialog_roots: Vec::new(),
            sub_choices: Vec::new(),
            selected_index: 0,
            mode: DialogUiMode::default(),
            npc_name: String::new(),
            rows_dirty: false,
        }
    }
}

impl DialogUiState {
    /// Dialog is open while the npc entity is present
    pub(super) fn is_open(&self) -> bool {
        self.npc_entity.is_some()
    }

    /// Opens the dialog with the given npc
    pub(super) fn open(
        &mut self,
        npc_entity: Entity,
        npc_self: InstanceId,
        npc_talk_to: InstanceId,
        dialog_roots: Vec<DialogRoot>,
        npc_name: String,
    ) {
        self.npc_entity = Some(npc_entity);
        self.npc_player = Some(npc_self);
        self.npc_talk_to = Some(npc_talk_to);
        self.dialog_roots = dialog_roots;
        self.sub_choices.clear();
        self.reset_to_options();
        self.npc_name = npc_name;
        self.rows_dirty = true;
    }

    /// Options currently shown: sub dialog choices when present, root options otherwise
    pub(super) fn active_choices(&self) -> &Vec<DialogRoot> {
        if self.sub_choices.is_empty() {
            &self.dialog_roots
        } else {
            &self.sub_choices
        }
    }

    /// The line currently being played back in lines mode, if any
    pub(super) fn current_line(&self) -> Option<&DialogLineView> {
        let DialogUiMode::Lines { lines, index, .. } = &self.mode else {
            return None;
        };
        lines.get(*index)
    }

    /// Whether root/sub options are currently shown
    pub(super) fn is_showing_options(&self) -> bool {
        matches!(self.mode, DialogUiMode::Options)
    }

    /// Whether dialog lines are currently played back
    pub(super) fn is_playing_lines(&self) -> bool {
        matches!(self.mode, DialogUiMode::Lines { .. })
    }

    /// Moves the selection by one entry, wrapping around at both ends
    pub(super) fn move_selection(&mut self, delta: i32) {
        let count = self.active_choices().len() as i32;
        if count == 0 {
            self.selected_index = 0;
            return;
        }
        self.selected_index = (self.selected_index as i32 + delta).rem_euclid(count) as usize;
    }

    /// Selected option
    fn selected_root(&self) -> Option<&DialogRoot> {
        self.active_choices().get(self.selected_index)
    }

    /// Runs the selected option sequence and starts displaying the produced lines.
    /// Keeps options shown when nothing was produced.
    pub(super) fn execute_selected(
        &mut self,
        dialog_texts: &DialogTexts,
        speech_sounds: &SpeechSounds,
        sounds: &mut MessageWriter<PlaySound>,
    ) {
        let Some(root) = self.selected_root() else {
            self.close();
            return;
        };
        let Some(npc_player) = self.npc_player else {
            return;
        };
        let Some(npc_talk_to) = self.npc_talk_to else {
            return;
        };
        let sequence_function_id = root.sequence_function_id;
        let info_instance_id = root.info_instance_id;

        let dialog_line_views = {
            let mut gothic_script = self.script.lock();
            gothic_script.mark_info_known(npc_player, info_instance_id);
            // Sub choices are cleared before running a root level option
            gothic_script.clear_sub_dialog_choices();
            gothic_script.clear_dialog_lines();
            // Dialog functions are executed with `self` bound to the npc the dialog
            // is open with, like the original engine does for every dialog call
            gothic_script.set_self(npc_talk_to);
            gothic_script.set_other(npc_player);
            gothic_script.set_instance(npc_talk_to);
            // TODO call set_self and set_instance in call_dialog_sequence_function
            gothic_script.call_dialog_sequence_function(sequence_function_id);

            // The executed option may have registered sub dialog choices, take a
            // snapshot so they can be shown instead of the root options
            self.sub_choices = gothic_script.sub_dialog_choices().to_vec();
            self.rows_dirty = true;

            build_line_views(gothic_script.dialog_lines(), dialog_texts, speech_sounds)
        };

        if dialog_line_views.is_empty() {
            // Nothing was produced (e.g. skipped sequence function), keep options shown
            self.refresh_root_options();
            return;
        }
        self.begin_line_playback(dialog_line_views, sounds);
    }

    /// Re-fetches the npc's root options from the script state, as executed
    /// options may have changed which options are available. Skipped while sub
    /// dialog choices are shown, they stay until one of their options runs.
    fn refresh_root_options(&mut self) {
        if !self.sub_choices.is_empty() {
            return;
        }
        let (Some(npc_player), Some(npc_talk_to)) = (self.npc_player, self.npc_talk_to) else {
            return;
        };
        let mut gothic_script = self.script.lock();
        let close_dialog = gothic_script.close_dialog();
        let roots = if close_dialog {
            None
        } else {
            super::npc_dialog_roots(&mut gothic_script, npc_player, npc_talk_to)
        };
        // The guard borrows `self`, drop it before mutating the state
        drop(gothic_script);
        if close_dialog {
            self.close();
            return;
        }
        if let Some(roots) = roots {
            self.dialog_roots = roots;
            if self.selected_index >= self.dialog_roots.len() {
                self.selected_index = 0;
            }
            self.rows_dirty = true;
        }
    }

    /// Closes the dialog by clearing the npc entity
    fn close(&mut self) {
        self.npc_entity = None;
        self.sub_choices.clear();
        self.reset_to_options();
    }

    /// Starts the one line at a time playback of the given lines
    fn begin_line_playback(
        &mut self,
        lines: Vec<DialogLineView>,
        sounds: &mut MessageWriter<PlaySound>,
    ) {
        self.mode = DialogUiMode::Lines {
            lines,
            index: 0,
            timer: Timer::default(),
        };
        self.start_current_line(sounds);
    }

    /// Restarts the display timer of the current line and plays its sound
    fn start_current_line(&mut self, sounds: &mut MessageWriter<PlaySound>) {
        let DialogUiMode::Lines {
            lines,
            index,
            timer,
        } = &mut self.mode
        else {
            return;
        };
        let Some(line) = lines.get(*index) else {
            return;
        };
        *timer = Timer::from_seconds(line_display_seconds(line), TimerMode::Once);
        sounds.write(PlaySound {
            asset_path: get_dialog_sound_path(&line.line.line_str),
        });
    }

    /// Returns the ui to options mode without any line playback
    fn reset_to_options(&mut self) {
        self.mode = DialogUiMode::Options;
        self.selected_index = 0;
    }

    /// Advances the line playback timer, the next line starts when it finishes
    pub(super) fn advance_playback(
        &mut self,
        delta: Duration,
        sounds: &mut MessageWriter<PlaySound>,
    ) {
        let DialogUiMode::Lines { timer, .. } = &mut self.mode else {
            return;
        };
        timer.tick(delta);
        if !timer.is_finished() {
            return;
        }
        self.finish_current_line(sounds);
    }

    /// Ends the current line early, the next one starts immediately. After the
    /// last line the options are shown again
    pub(super) fn stop_current_line(&mut self, sounds: &mut MessageWriter<PlaySound>) {
        self.finish_current_line(sounds);
    }

    /// Starts the next line after the current one ended, or returns to options
    /// mode after the last line. When returning to the root options the roots
    /// are re-fetched from the script state, as the executed option may have
    /// changed which options are available
    fn finish_current_line(&mut self, sounds: &mut MessageWriter<PlaySound>) {
        let DialogUiMode::Lines { lines, index, .. } = &mut self.mode else {
            return;
        };
        let next_index = *index + 1;
        if next_index >= lines.len() {
            // Playback finished, show root options again
            self.reset_to_options();
            self.refresh_root_options();
            return;
        }
        *index = next_index;
        self.start_current_line(sounds);
    }
}

/// Options mode input: navigation and activation. In lines mode space ends
/// the current line early
pub(super) fn handle_dialog_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut dialog_ui: ResMut<DialogUiState>,
    dialog_texts: Res<DialogTexts>,
    speech_sounds: Res<SpeechSounds>,
    mut sounds: MessageWriter<PlaySound>,
    mut sound_manager: ResMut<SoundManager>,
    mut commands: Commands,
) {
    if !dialog_ui.is_open() {
        return;
    }

    if !dialog_ui.is_showing_options() {
        if keys.just_pressed(KeyCode::Space) {
            dialog_ui.stop_current_line(&mut sounds);
            sound_manager.stop_active(&mut commands);
        }
        return;
    }

    if keys.just_pressed(KeyCode::ArrowUp) {
        dialog_ui.move_selection(-1);
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        dialog_ui.move_selection(1);
    }
    if keys.just_pressed(KeyCode::Enter) {
        dialog_ui.execute_selected(&dialog_texts, &speech_sounds, &mut sounds);
    }
}

/// Resolves display text and sound duration for the script's dialog lines
fn build_line_views(
    lines: &[DialogLine],
    dialog_texts: &DialogTexts,
    speech_sounds: &SpeechSounds,
) -> Vec<DialogLineView> {
    lines
        .iter()
        .map(|line| {
            let text = dialog_texts.get(&line.line_str).cloned();
            if text.is_none() {
                warn!(
                    "Missing dialog text: npc_a({}) npc_b({}) line_id({}) block({})",
                    line.npc_a, line.npc_b, line.line_id, line.line_str
                );
            }
            let sound_duration = speech_sounds.duration(&get_dialog_sound_path(&line.line_str));
            DialogLineView {
                line: line.clone(),
                text,
                sound_duration,
            }
        })
        .collect()
}

/// Display time of a single dialog line: sound duration with a gap after
/// it, but at least the minimum display time
fn line_display_seconds(view: &DialogLineView) -> f32 {
    match view.sound_duration {
        Some(duration) => (duration + LINE_GAP_SECONDS).max(LINE_DISPLAY_SECONDS),
        None => LINE_DISPLAY_SECONDS,
    }
}

/// Advances the one line at a time playback
pub(super) fn advance_dialog_playback(
    time: Res<Time>,
    mut dialog_ui: ResMut<DialogUiState>,
    mut sounds: MessageWriter<PlaySound>,
) {
    if !dialog_ui.is_open() {
        return;
    }
    dialog_ui.advance_playback(time.delta(), &mut sounds);
}
