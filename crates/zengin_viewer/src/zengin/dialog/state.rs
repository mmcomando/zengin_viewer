use crate::audio::{PlaySound, SoundManager};
use crate::input::{GuiAction, GuiInputMarker};
use crate::zengin::script::symbol_id::{FunctionId, InstanceId, StringId};
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;
use std::time::Duration;

/// How long a single dialog line is displayed during playback
const LINE_DISPLAY_SECONDS: f32 = 1.0;

/// Extra pause after a line's sound finished before the next line is shown
const LINE_GAP_SECONDS: f32 = 0.2;

/// A root or sub dialog choice prepared for display and for being sent back when
/// picked.
#[derive(Debug, Clone)]
pub(super) struct DialogOptionView {
    pub(super) description: String,
    pub(super) sequence_function_id: FunctionId,
    pub(super) info_instance_id: Option<InstanceId>,
}

/// A dialog line prepared for display, with the real text and sound resolved
#[derive(Debug, Clone)]
pub(super) struct DialogLineView {
    /// Real line text, `None` if the block was not found
    pub(super) text: Option<String>,
    /// zengin asset path of the line's sound
    pub(super) sound_path: String,
    /// Duration of the line's sound file, `None` if not found/parseable
    pub(super) sound_duration: Option<f32>,
    /// Ids kept only for the "missing text" debug line
    pub(super) npc_a: InstanceId,
    pub(super) npc_b: InstanceId,
    pub(super) line_id: StringId,
}

/// Asks the script to act on the currently open dialog. Written by the dialog UI
/// systems, applied by `handle_dialog_requests` in the same frame, so no reply
/// message or correlation is needed.
#[derive(Message)]
pub(super) enum DialogRequest {
    /// Open the dialog with the npc, showing its current root options
    Open {
        npc_entity: Entity,
        npc_from: InstanceId,
        npc_talk_to: InstanceId,
        npc_name: String,
    },
    /// Open the dialog with the npc and immediately play one of its important
    /// infos, used by the npc initiated dialogs
    OpenImportant {
        npc_entity: Entity,
        npc_from: InstanceId,
        npc_talk_to: InstanceId,
        sequence_function_id: FunctionId,
        info_instance_id: Option<InstanceId>,
    },
    /// Refresh the root options, e.g. after a line finished playing
    RefreshRoots {
        npc_from: InstanceId,
        npc_talk_to: InstanceId,
    },
    /// Run the picked option sequence and show the produced lines/sub choices
    ChooseOption {
        npc_from: InstanceId,
        npc_talk_to: InstanceId,
        sequence_function_id: FunctionId,
        info_instance_id: Option<InstanceId>,
    },
}

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

/// State driving the dialog UI, it is shown only while `npc_entity` is present
#[derive(Resource, Default)]
pub(crate) struct DialogUiState {
    pub(super) npc_entity: Option<Entity>,
    /// Player character (visual) entity, used to aim the dialog camera when the
    /// player is the one talking
    pub(super) player_entity: Option<Entity>,
    pub(super) npc_player: Option<InstanceId>,
    /// Script instance of the npc the dialog is open with
    pub(super) npc_talk_to: Option<InstanceId>,
    pub(super) dialog_roots: Vec<DialogOptionView>,
    /// Sub dialog choices populated by the last executed option, shown instead
    /// of `dialog_roots` when not empty
    pub(super) sub_choices: Vec<DialogOptionView>,
    /// Highlighted entry in options mode
    pub(super) selected_index: usize,
    mode: DialogUiMode,
    /// Symbol name of the npc the dialog is open with, resolved when the dialog opens
    pub(super) npc_name: String,
    /// Set when the shown options changed and the option rows have to be re-synced
    pub(super) rows_dirty: bool,
    /// Set when the script asked to end the dialog while lines were still being
    /// played, the dialog closes once the playback finished
    pending_close: bool,
    /// Set while waiting for the outputs of the option that is currently being
    /// executed, consumed by `handle_dialog_outputs` when they arrive
    awaiting_outputs: bool,
}

impl DialogUiState {
    /// Dialog is open while the npc entity is present
    pub(crate) fn is_open(&self) -> bool {
        self.npc_entity.is_some()
    }

    /// Opens the dialog with the given npc and its root options
    pub(super) fn open(
        &mut self,
        npc_entity: Entity,
        player_entity: Entity,
        npc_self: InstanceId,
        npc_talk_to: InstanceId,
        npc_name: String,
        options: Vec<DialogOptionView>,
    ) {
        self.npc_entity = Some(npc_entity);
        self.player_entity = Some(player_entity);
        self.npc_player = Some(npc_self);
        self.npc_talk_to = Some(npc_talk_to);
        self.dialog_roots = options;
        self.sub_choices.clear();
        self.reset_to_options();
        self.npc_name = npc_name;
        self.rows_dirty = true;
        self.pending_close = false;
        self.awaiting_outputs = false;
    }

    /// Pair of `(viewed, listener)` character entities for the dialog camera. The
    /// viewed one is centered by the camera (the speaker of the line being
    /// played, or the npc we talk to while the options are shown), the listener
    /// is the other one, only used for the camera distance.
    pub(super) fn camera_characters(&self) -> Option<(Entity, Entity)> {
        let viewing_player = self.current_line().map(|line| line.npc_a) == self.npc_player;
        let (viewed, listener) = if viewing_player {
            (self.player_entity?, self.npc_entity?)
        } else {
            (self.npc_entity?, self.player_entity?)
        };
        Some((viewed, listener))
    }

    /// Replaces the shown root options
    pub(super) fn accept_options(&mut self, options: Vec<DialogOptionView>) {
        self.dialog_roots = options;
        if self.selected_index >= self.dialog_roots.len() {
            self.selected_index = 0;
        }
        self.rows_dirty = true;
    }

    /// Replaces the shown sub dialog choices
    pub(super) fn accept_sub_choices(&mut self, sub_choices: Vec<DialogOptionView>) {
        self.sub_choices = sub_choices;
        self.rows_dirty = true;
    }

    /// Options currently shown: sub dialog choices when present, root options otherwise
    pub(super) fn active_choices(&self) -> &Vec<DialogOptionView> {
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
    fn selected_choice(&self) -> Option<&DialogOptionView> {
        self.active_choices().get(self.selected_index)
    }

    /// Asks the script to run the selected option. Closes the dialog when there
    /// is no selected option.
    pub(super) fn execute_selected(&mut self, requests: &mut MessageWriter<DialogRequest>) {
        let Some((sequence_function_id, info_instance_id)) = self
            .selected_choice()
            .map(|choice| (choice.sequence_function_id, choice.info_instance_id))
        else {
            self.close();
            return;
        };
        let (Some(npc_from), Some(npc_talk_to)) = (self.npc_player, self.npc_talk_to) else {
            return;
        };
        requests.write(DialogRequest::ChooseOption {
            npc_from,
            npc_talk_to,
            sequence_function_id,
            info_instance_id,
        });
    }

    /// Asks the script for the up to date root options. Skipped while sub dialog
    /// choices are shown, they stay until one of their options runs.
    pub(super) fn refresh_root_options(&mut self, requests: &mut MessageWriter<DialogRequest>) {
        if !self.sub_choices.is_empty() {
            return;
        }
        let (Some(npc_from), Some(npc_talk_to)) = (self.npc_player, self.npc_talk_to) else {
            return;
        };
        requests.write(DialogRequest::RefreshRoots {
            npc_from,
            npc_talk_to,
        });
    }

    /// Closes the dialog by clearing the npc entity
    pub(crate) fn close(&mut self) {
        self.npc_entity = None;
        self.sub_choices.clear();
        self.pending_close = false;
        self.reset_to_options();
    }

    /// Requests the dialog to end, immediately when no lines are playing,
    /// otherwise after the line playback finished
    pub(super) fn request_close(&mut self) {
        if self.is_playing_lines() {
            self.pending_close = true;
        } else {
            self.close();
        }
    }

    /// Arms the capture of the outputs produced by the option being executed
    pub(super) fn begin_output_capture(&mut self) {
        self.awaiting_outputs = true;
    }

    /// Returns and clears whether outputs of an executed option are awaited
    pub(super) fn take_awaiting_outputs(&mut self) -> bool {
        std::mem::take(&mut self.awaiting_outputs)
    }

    /// Starts the one line at a time playback of the given lines
    pub(super) fn begin_line_playback(
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
            asset_path: line.sound_path.clone(),
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
        requests: &mut MessageWriter<DialogRequest>,
    ) {
        let DialogUiMode::Lines { timer, .. } = &mut self.mode else {
            return;
        };
        timer.tick(delta);
        if !timer.is_finished() {
            return;
        }
        self.finish_current_line(sounds, requests);
    }

    /// Ends the current line early, the next one starts immediately. After the
    /// last line the options are shown again
    pub(super) fn stop_current_line(
        &mut self,
        sounds: &mut MessageWriter<PlaySound>,
        requests: &mut MessageWriter<DialogRequest>,
    ) {
        self.finish_current_line(sounds, requests);
    }

    /// Starts the next line after the current one ended, or returns to options
    /// mode after the last line. When returning to the root options they are
    /// re-requested from the script, as the executed option may have changed
    /// which options are available
    fn finish_current_line(
        &mut self,
        sounds: &mut MessageWriter<PlaySound>,
        requests: &mut MessageWriter<DialogRequest>,
    ) {
        let DialogUiMode::Lines { lines, index, .. } = &mut self.mode else {
            return;
        };
        let next_index = *index + 1;
        if next_index >= lines.len() {
            self.reset_to_options();
            if self.pending_close {
                self.close();
                return;
            }
            // Playback finished, show root options again
            self.refresh_root_options(requests);
            return;
        }
        *index = next_index;
        self.start_current_line(sounds);
    }
}

/// Options mode input: navigation and activation. In lines mode space ends
/// the current line early
pub(super) fn handle_dialog_input(
    action_state: Single<&ActionState<GuiAction>, With<GuiInputMarker>>,
    mut dialog_ui: ResMut<DialogUiState>,
    mut requests: MessageWriter<DialogRequest>,
    mut sounds: MessageWriter<PlaySound>,
    mut sound_manager: ResMut<SoundManager>,
    mut commands: Commands,
) {
    if !dialog_ui.is_open() {
        return;
    }

    if !dialog_ui.is_showing_options() {
        if action_state.just_pressed(&GuiAction::CancelAction) {
            dialog_ui.stop_current_line(&mut sounds, &mut requests);
            sound_manager.stop_active(&mut commands);
        }
        return;
    }

    if action_state.just_pressed(&GuiAction::Up) {
        dialog_ui.move_selection(-1);
    }
    if action_state.just_pressed(&GuiAction::Down) {
        dialog_ui.move_selection(1);
    }
    if action_state.just_pressed(&GuiAction::Confirm) {
        dialog_ui.execute_selected(&mut requests);
    }
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
    mut requests: MessageWriter<DialogRequest>,
) {
    if !dialog_ui.is_open() {
        return;
    }
    dialog_ui.advance_playback(time.delta(), &mut sounds, &mut requests);
}
