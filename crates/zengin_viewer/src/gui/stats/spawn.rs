use bevy::prelude::*;

use crate::perf_debug;
use crate::ui_kit::spawn_gui_root;

#[derive(Component)]
pub(super) struct StatsRootMarker;

#[derive(Component)]
pub(super) struct StatsContentMarker;

pub(super) fn spawn_stats_ui(mut commands: Commands) {
    let _perf = perf_debug!();
    let root = spawn_gui_root(&mut commands, StatsRootMarker);
    commands.entity(root).insert(GlobalZIndex(10));
    commands.spawn((
        ChildOf(root),
        StatsContentMarker,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
    ));
}
