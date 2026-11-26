use bevy::prelude::*;

use dev_util::ui_kit::spawn_gui_root;

use super::state::JournalCategory;
use dev_util::perf_debug;

#[derive(Component)]
pub struct JournalRootMarker;

#[derive(Component)]
pub enum JournalContainerMarker {
    Main,
    Detail,
}

#[derive(Component)]
pub struct JournalListMarker;

#[derive(Component)]
pub struct JournalDetailScrollMarker;

/// Text node kind, so one query can update all journal texts without aliasing.
#[derive(Component)]
pub enum JournalTextMarker {
    /// MENU.DAT title text that never changes.
    Static,
    Category(JournalCategory),
    Day,
    Time,
    Row(usize),
    Detail,
    DetailFooter,
}

pub fn spawn_journal_root(mut commands: Commands) {
    let _perf = perf_debug!();
    let root = spawn_gui_root(&mut commands, JournalRootMarker);
    commands.entity(root).insert(GlobalZIndex(10));
}
