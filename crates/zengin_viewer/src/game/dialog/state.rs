use crate::game::audio::{PlaySound, SoundManager};
use crate::zengin::common::get_dialog_sound_path;
use crate::zengin::script::component::{DialogTexts, WorldScript};
use crate::zengin::script::state::{DialogLine, DialogRoot};
use crate::zengin::sounds::SpeechSounds;
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
#[derive(Resource, Default)]
pub(super) struct DialogUiState {
    pub(super) npc_entity: Option<Entity>,
    /// Script instance id of the npc the dialog is open with
    pub(super) npc_instance_id: i32,
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

impl DialogUiState {
    /// Dialog is open while the npc entity is present
    pub(super) fn is_open(&self) -> bool {
        self.npc_entity.is_some()
    }

    /// Opens the dialog with the given npc
    pub(super) fn open(
        &mut self,
        npc_entity: Entity,
        npc_instance_id: i32,
        dialog_roots: Vec<DialogRoot>,
        npc_name: String,
    ) {
        self.npc_entity = Some(npc_entity);
        self.npc_instance_id = npc_instance_id;
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

    /// Sequence function id of the selected option
    fn selected_option_function(&self) -> Option<i32> {
        self.active_choices()
            .get(self.selected_index)
            .map(|root| root.sequence_function_id)
    }

    /// Runs the selected option sequence and starts displaying the produced lines.
    /// Keeps options shown when nothing was produced.
    pub(super) fn execute_selected(
        &mut self,
        gothic_script: &mut WorldScript,
        dialog_texts: &DialogTexts,
        speech_sounds: &SpeechSounds,
        sounds: &mut MessageWriter<PlaySound>,
    ) {
        let Some(sequence_function_id) = self.selected_option_function() else {
            self.close();
            return;
        };

        // Sub choices are cleared before running a root level option
        gothic_script.clear_sub_dialog_choices();
        gothic_script.clear_dialog_lines();
        // Dialog functions are executed with `self` bound to the npc the dialog
        // is open with, like the original engine does for every dialog call
        gothic_script.set_self(self.npc_instance_id);
        gothic_script.set_instance(self.npc_instance_id);
        // TODO call set_self and set_instance in call_dialog_sequence_function
        gothic_script.call_dialog_sequence_function(sequence_function_id);

        // The executed option may have registered sub dialog choices, take a
        // snapshot so they can be shown instead of the root options
        self.sync_sub_choices(gothic_script);

        let dialog_line_views =
            build_line_views(gothic_script.dialog_lines(), dialog_texts, speech_sounds);

        if dialog_line_views.is_empty() {
            // Nothing was produced (e.g. skipped sequence function), keep options shown
            self.refresh_root_options(gothic_script);
            return;
        }
        self.begin_line_playback(dialog_line_views, sounds);
    }

    /// Takes the sub dialog choices produced by the executed option, if any
    pub(super) fn sync_sub_choices(&mut self, gothic_script: &WorldScript) {
        self.sub_choices = gothic_script.sub_dialog_choices().to_vec();
        self.rows_dirty = true;
    }

    /// Re-fetches the npc's root options from the script state, as executed
    /// options may have changed which options are available. Skipped while sub
    /// dialog choices are shown, they stay until one of their options runs.
    fn refresh_root_options(&mut self, gothic_script: &mut WorldScript) {
        if !self.sub_choices.is_empty() {
            return;
        }
        if gothic_script.close_dialog() {
            self.close();
            return;
        }
        if let Some(roots) = super::npc_dialog_roots(gothic_script, self.npc_instance_id) {
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
        gothic_script: &mut WorldScript,
        sounds: &mut MessageWriter<PlaySound>,
    ) {
        let DialogUiMode::Lines { timer, .. } = &mut self.mode else {
            return;
        };
        timer.tick(delta);
        if !timer.is_finished() {
            return;
        }
        self.finish_current_line(gothic_script, sounds);
    }

    /// Ends the current line early, the next one starts immediately. After the
    /// last line the options are shown again
    pub(super) fn stop_current_line(
        &mut self,
        gothic_script: &mut WorldScript,
        sounds: &mut MessageWriter<PlaySound>,
    ) {
        self.finish_current_line(gothic_script, sounds);
    }

    /// Starts the next line after the current one ended, or returns to options
    /// mode after the last line. When returning to the root options the roots
    /// are re-fetched from the script state, as the executed option may have
    /// changed which options are available
    fn finish_current_line(
        &mut self,
        gothic_script: &mut WorldScript,
        sounds: &mut MessageWriter<PlaySound>,
    ) {
        let DialogUiMode::Lines { lines, index, .. } = &mut self.mode else {
            return;
        };
        let next_index = *index + 1;
        if next_index >= lines.len() {
            // Playback finished, show root options again
            self.reset_to_options();
            self.refresh_root_options(gothic_script);
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
    mut gothic_script: ResMut<WorldScript>,
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
            dialog_ui.stop_current_line(&mut gothic_script, &mut sounds);
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
        dialog_ui.execute_selected(
            &mut gothic_script,
            &dialog_texts,
            &speech_sounds,
            &mut sounds,
        );
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
                println!(
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
    mut gothic_script: ResMut<WorldScript>,
    mut sounds: MessageWriter<PlaySound>,
) {
    if !dialog_ui.is_open() {
        return;
    }
    dialog_ui.advance_playback(time.delta(), &mut gothic_script, &mut sounds);
}
