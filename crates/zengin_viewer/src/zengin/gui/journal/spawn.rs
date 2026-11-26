use bevy::prelude::*;

use dev_util::ui_kit::spawn_gui_root;

use super::state::JournalCategory;
use dev_util::perf_debug;

#[derive(Component)]
pub(super) struct JournalRootMarker;

#[derive(Component)]
pub(super) enum JournalContainerMarker {
    Main,
    Detail,
}

#[derive(Component)]
pub(super) struct JournalListMarker;

#[derive(Component)]
pub(super) struct JournalDetailScrollMarker;

/// Text node kind, so one query can update all journal texts without aliasing.
#[derive(Component)]
pub(super) enum JournalTextMarker {
    /// MENU.DAT title text that never changes.
    Static,
    Category(JournalCategory),
    Day,
    Time,
    Row(usize),
    Detail,
    DetailFooter,
}

pub(super) fn spawn_journal_root(mut commands: Commands) {
    let _perf = perf_debug!();
    let root = spawn_gui_root(&mut commands, JournalRootMarker);
    commands.entity(root).insert(GlobalZIndex(10));
}
