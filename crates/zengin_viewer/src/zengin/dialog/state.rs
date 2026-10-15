use crate::audio::sound_manager::{PlaySound, SoundManager};
use crate::zengin::script::symbol_id::{FunctionId, InstanceId, StringId};
use bevy::prelude::*;
use std::time::Duration;

/// How long a single dialog line is displayed during playback
const LINE_DISPLAY_SECONDS: f32 = 1.0;

/// Extra pause after a line's sound finished before the next line is shown
const LINE_GAP_SECONDS: f32 = 0.2;

/// A root or sub dialog choice prepared for display and for being sent back when
/// picked.
#[derive(Debug, Clone)]
pub(crate) struct DialogOptionView {
    pub(crate) description: String,
    pub(crate) sequence_function_id: FunctionId,
    pub(crate) info_instance_id: Option<InstanceId>,
}

/// A dialog line prepared for display, with the real text and sound resolved
#[derive(Debug, Clone)]
pub(crate) struct DialogLineView {
    /// Real line text, `None` if the block was not found
    pub(crate) text: Option<String>,
    /// zengin asset path of the line's sound
    pub(crate) sound_path: String,
    /// Duration of the line's sound file, `None` if not found/parseable
    pub(crate) sound_duration: Option<f32>,
    /// Ids kept only for the "missing text" debug line
    pub(crate) npc_a: InstanceId,
    pub(crate) npc_b: InstanceId,
    pub(crate) line_id: StringId,
}

/// Requests the script to produce dialog data for the currently open dialog.
/// Written by the dialog UI, handled by `handle_dialog_requests` in the bridge.
#[derive(Message)]
pub(crate) enum DialogRequest {
    /// Open the dialog with the npc and reply with its current root options
    Open {
        generation: u32,
        npc_from: InstanceId,
        npc_talk_to: InstanceId,
    },
    /// Reply with the current root options, e.g. after a line finished playing
    RefreshRoots {
        generation: u32,
        npc_from: InstanceId,
        npc_talk_to: InstanceId,
    },
    /// Run the picked option sequence and reply with the produced lines/sub choices
    ChooseOption {
        generation: u32,
        npc_from: InstanceId,
        npc_talk_to: InstanceId,
        sequence_function_id: FunctionId,
        info_instance_id: Option<InstanceId>,
    },
}

/// Reply produced by the script bridge for a [`DialogRequest`]. A reply whose
/// `generation` differs from the current one (dialog closed or reopened in the
/// meantime) is ignored by the UI.
#[derive(Message)]
pub(crate) enum DialogReply {
    /// Root options, `None` closes the dialog (exit / npc has no dialog)
    Options {
        generation: u32,
        options: Option<Vec<DialogOptionView>>,
    },
    /// Lines produced by the picked option and possibly sub choices; empty
    /// `lines` means only the (sub) options should be shown
    Lines {
        generation: u32,
        lines: Vec<DialogLineView>,
        sub_choices: Vec<DialogOptionView>,
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
pub(super) struct DialogUiState {
    pub(super) npc_entity: Option<Entity>,
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
    /// Bumped on every open/close, replies with a different value are stale
    pub(super) generation: u32,
    /// A root-options reply is expected, on arrival the npc and player face each
    /// other and the dialog camera is set
    pub(super) awaiting_open: bool,
    /// A request is in flight, input that would issue another one is ignored
    pub(super) pending: bool,
}

impl DialogUiState {
    /// Dialog is open while the npc entity is present
    pub(super) fn is_open(&self) -> bool {
        self.npc_entity.is_some()
    }

    /// Starts opening the dialog with the given npc; its options arrive with the
    /// script reply. Returns the generation to tag the [`DialogRequest::Open`].
    pub(super) fn open(
        &mut self,
        npc_entity: Entity,
        npc_self: InstanceId,
        npc_talk_to: InstanceId,
        npc_name: String,
    ) -> u32 {
        self.generation = self.generation.wrapping_add(1);
        self.npc_entity = Some(npc_entity);
        self.npc_player = Some(npc_self);
        self.npc_talk_to = Some(npc_talk_to);
        self.dialog_roots.clear();
        self.sub_choices.clear();
        self.reset_to_options();
        self.npc_name = npc_name;
        self.rows_dirty = true;
        self.awaiting_open = true;
        self.pending = true;
        self.generation
    }

    /// Applies the root options reply, replacing the shown options
    pub(super) fn accept_options(&mut self, options: Vec<DialogOptionView>) {
        self.dialog_roots = options;
        if self.selected_index >= self.dialog_roots.len() {
            self.selected_index = 0;
        }
        self.rows_dirty = true;
        self.awaiting_open = false;
        self.pending = false;
    }

    /// Applies the sub choices of a lines reply
    pub(super) fn accept_sub_choices(&mut self, sub_choices: Vec<DialogOptionView>) {
        self.sub_choices = sub_choices;
        self.rows_dirty = true;
        self.pending = false;
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

    /// Sends the selected option to the script, the produced lines arrive with
    /// the reply. Closes the dialog when there is no selected option.
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
            generation: self.generation,
            npc_from,
            npc_talk_to,
            sequence_function_id,
            info_instance_id,
        });
        self.pending = true;
    }

    /// Requests the up to date root options from the script. Skipped while sub
    /// dialog choices are shown, they stay until one of their options runs.
    pub(super) fn refresh_root_options(&mut self, requests: &mut MessageWriter<DialogRequest>) {
        if !self.sub_choices.is_empty() {
            return;
        }
        let (Some(npc_from), Some(npc_talk_to)) = (self.npc_player, self.npc_talk_to) else {
            return;
        };
        requests.write(DialogRequest::RefreshRoots {
            generation: self.generation,
            npc_from,
            npc_talk_to,
        });
        self.pending = true;
    }

    /// Closes the dialog by clearing the npc entity, invalidating in-flight replies
    pub(super) fn close(&mut self) {
        self.npc_entity = None;
        self.sub_choices.clear();
        self.reset_to_options();
        self.awaiting_open = false;
        self.pending = false;
        self.generation = self.generation.wrapping_add(1);
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
            // Playback finished, show root options again
            self.reset_to_options();
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
    keys: Res<ButtonInput<KeyCode>>,
    mut dialog_ui: ResMut<DialogUiState>,
    mut requests: MessageWriter<DialogRequest>,
    mut sounds: MessageWriter<PlaySound>,
    mut sound_manager: ResMut<SoundManager>,
    mut commands: Commands,
) {
    if !dialog_ui.is_open() {
        return;
    }
    // A request is in flight, do not issue another one
    if dialog_ui.pending {
        return;
    }

    if !dialog_ui.is_showing_options() {
        if keys.just_pressed(KeyCode::Space) {
            dialog_ui.stop_current_line(&mut sounds, &mut requests);
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
