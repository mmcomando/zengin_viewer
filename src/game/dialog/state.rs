use crate::zengin::script::component::{DialogTexts, WorldScript};
use crate::zengin::script::state::{DialogLine, DialogRoot};
use bevy::prelude::*;
use std::time::Duration;

/// How long a single dialog line is displayed during playback
const LINE_DISPLAY_SECONDS: f32 = 1.0;

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
}

/// State driving the dialog UI, it is shown only while `npc_entity` is present
#[derive(Resource, Default)]
pub(super) struct DialogUiState {
    pub(super) npc_entity: Option<Entity>,
    pub(super) dialog_roots: Vec<DialogRoot>,
    pub(super) dialog_lines: Vec<DialogLineView>,
    pub(super) mode: DialogUiMode,
    /// Highlighted entry in options mode, EXIT is at index `dialog_roots.len()`
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

    /// Index of the EXIT entry, it is right after the last root option
    pub(super) fn exit_index(&self) -> usize {
        self.dialog_roots.len()
    }

    /// Opens the dialog with the given npc
    pub(super) fn open(
        &mut self,
        npc_entity: Entity,
        mut dialog_roots: Vec<DialogRoot>,
        npc_name: String,
    ) {
        // Important roots should not be shown, they are automatically started when player is near
        dialog_roots.retain(|el| !el.important);

        self.npc_entity = Some(npc_entity);
        self.dialog_roots = dialog_roots;
        self.dialog_lines.clear();
        self.mode = DialogUiMode::Options;
        self.selected_index = 0;
        self.current_line_index = 0;
        self.npc_name = npc_name;
        self.rows_dirty = true;
    }

    /// Moves the selection by one entry, clamped to the available range including EXIT
    pub(super) fn move_selection(&mut self, delta: i32) {
        let selected = self.selected_index as i32 + delta;
        self.selected_index = selected.clamp(0, self.exit_index() as i32) as usize;
    }

    /// Sequence function id of the selected option, None when EXIT is selected
    fn selected_option_function(&self) -> Option<i32> {
        if self.selected_index == self.exit_index() {
            return None;
        }
        Some(self.dialog_roots[self.selected_index].sequence_function_id)
    }

    /// Runs the selected option sequence and starts displaying the produced lines.
    /// Closes the dialog when EXIT is selected, keeps options shown when nothing was produced.
    pub(super) fn execute_selected(
        &mut self,
        gothic_script: &mut WorldScript,
        dialog_texts: &DialogTexts,
    ) {
        let Some(sequence_function_id) = self.selected_option_function() else {
            self.close();
            return;
        };

        gothic_script.clear_dialog_lines();
        gothic_script.call_dialog_sequence_function(sequence_function_id);

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
                DialogLineView { line, text }
            })
            .collect::<Vec<_>>();

        if dialog_line_views.is_empty() {
            // Nothing was produced (e.g. skipped sequence function), keep options shown
            return;
        }
        self.begin_line_playback(dialog_line_views);
    }

    /// Closes the dialog by clearing the npc entity
    fn close(&mut self) {
        self.npc_entity = None;
    }

    /// Starts the one line at a time playback of the given lines
    fn begin_line_playback(&mut self, dialog_lines: Vec<DialogLineView>) {
        self.dialog_lines = dialog_lines;
        self.mode = DialogUiMode::Lines;
        self.current_line_index = 0;
        self.line_timer = Timer::from_seconds(LINE_DISPLAY_SECONDS, TimerMode::Once);
    }

    /// Advances the line playback timer, returns to options mode after the last line
    pub(super) fn advance_playback(&mut self, delta: Duration) {
        if self.mode != DialogUiMode::Lines {
            return;
        }
        self.line_timer.tick(delta);
        if self.line_timer.is_finished() {
            self.current_line_index += 1;
            self.line_timer = Timer::from_seconds(LINE_DISPLAY_SECONDS, TimerMode::Once);
            if self.current_line_index >= self.dialog_lines.len() {
                // Playback finished, show root options again
                self.mode = DialogUiMode::Options;
                self.current_line_index = 0;
                self.selected_index = 0;
                self.dialog_lines.clear();
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
        dialog_ui.execute_selected(&mut gothic_script, &dialog_texts);
    }
}

/// Advances the one line at a time playback
pub(super) fn advance_dialog_playback(time: Res<Time>, mut dialog_ui: ResMut<DialogUiState>) {
    if !dialog_ui.is_open() {
        return;
    }
    dialog_ui.advance_playback(time.delta());
}
