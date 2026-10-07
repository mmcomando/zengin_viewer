use crate::game::audio::PlaySound;
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
#[derive(Default, PartialEq)]
pub(super) enum DialogUiMode {
    /// Root options of the npc, shown at the bottom center
    #[default]
    Options,
    /// Lines produced by the last executed option, shown one by one at the top
    Lines,
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
    pub(super) dialog_lines: Vec<DialogLineView>,
    pub(super) mode: DialogUiMode,
    /// Highlighted entry in options mode
    pub(super) selected_index: usize,
    /// Index of the currently displayed line in lines mode
    pub(super) current_line_index: usize,
    line_timer: Timer,
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
        self.dialog_lines.clear();
        self.mode = DialogUiMode::Options;
        self.selected_index = 0;
        self.current_line_index = 0;
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

    /// Moves the selection by one entry
    pub(super) fn move_selection(&mut self, delta: i32) {
        if self.active_choices().is_empty() {
            self.selected_index = 0;
            return;
        }
        let selected = self.selected_index as i32 + delta;
        self.selected_index = selected.clamp(0, (self.active_choices().len() - 1) as i32) as usize;
    }

    /// Sequence function id of the selected option
    fn selected_option_function(&self) -> Option<i32> {
        Some(
            self.active_choices()
                .get(self.selected_index)?
                .sequence_function_id,
        )
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
        gothic_script.call_dialog_sequence_function(sequence_function_id);

        // The executed option may have registered sub dialog choices, take a
        // snapshot so they can be shown instead of the root options
        self.sync_sub_choices(gothic_script);

        let dialog_line_views = gothic_script
            .dialog_lines()
            .to_vec()
            .into_iter()
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
                    line,
                    text,
                    sound_duration,
                }
            })
            .collect::<Vec<_>>();

        if dialog_line_views.is_empty() {
            // Nothing was produced (e.g. skipped sequence function), keep options shown
            if self.sub_choices.is_empty() {
                self.refresh_root_options(gothic_script);
            }
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
    /// options may have changed which options are available
    fn refresh_root_options(&mut self, gothic_script: &mut WorldScript) {
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
    }

    /// Starts the one line at a time playback of the given lines
    fn begin_line_playback(
        &mut self,
        dialog_lines: Vec<DialogLineView>,
        sounds: &mut MessageWriter<PlaySound>,
    ) {
        self.dialog_lines = dialog_lines;
        self.mode = DialogUiMode::Lines;
        self.current_line_index = 0;
        self.line_timer = self
            .dialog_lines
            .first()
            .map(|line| Timer::from_seconds(line_display_seconds(line), TimerMode::Once))
            .unwrap_or_default();

        if let Some(line) = self.dialog_lines.first() {
            sounds.write(PlaySound {
                asset_path: get_dialog_sound_path(&line.line.line_str),
            });
        }
    }

    /// Advances the line playback timer, returns to options mode after the last line.
    /// When returning to the root options the roots are re-fetched from the
    /// script state, as the executed option may have changed which options are
    /// available
    pub(super) fn advance_playback(
        &mut self,
        delta: Duration,
        gothic_script: &mut WorldScript,
        sounds: &mut MessageWriter<PlaySound>,
    ) {
        if self.mode != DialogUiMode::Lines {
            return;
        }
        self.line_timer.tick(delta);
        if self.line_timer.is_finished() {
            self.current_line_index += 1;
            if self.current_line_index >= self.dialog_lines.len() {
                // Playback finished, show root options again
                self.mode = DialogUiMode::Options;
                self.current_line_index = 0;
                self.selected_index = 0;
                self.dialog_lines.clear();

                if self.sub_choices.is_empty() {
                    self.refresh_root_options(gothic_script);
                }
            } else if let Some(line) = self.dialog_lines.get(self.current_line_index) {
                self.line_timer = Timer::from_seconds(line_display_seconds(line), TimerMode::Once);
                sounds.write(PlaySound {
                    asset_path: get_dialog_sound_path(&line.line.line_str),
                });
            }
        }
    }
}

/// Options mode input: navigation and activation
pub(super) fn handle_dialog_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut dialog_ui: ResMut<DialogUiState>,
    mut gothic_script: ResMut<WorldScript>,
    dialog_texts: Res<DialogTexts>,
    speech_sounds: Res<SpeechSounds>,
    mut sounds: MessageWriter<PlaySound>,
) {
    if !dialog_ui.is_open() || dialog_ui.mode != DialogUiMode::Options {
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
