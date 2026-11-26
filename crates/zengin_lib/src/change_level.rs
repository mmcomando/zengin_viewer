//! `oCTriggerChangeLevel` areas: entering one loads the target world and places
//! the player at its start vob.

use bevy::prelude::*;

use crate::AppState;
use crate::PlayerCharacter;
use crate::common::ChangeLevelTrigger;
use crate::world_plugin::begin_level_change;
use dev_util::perf_debug;

pub struct ChangeLevelPlugin;

impl Plugin for ChangeLevelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChangeLevelTriggers>();
        app.init_resource::<PendingChangeLevel>();
        app.add_systems(
            Update,
            detect_change_level.run_if(in_state(AppState::InGame)),
        );
        // The world teardown despawns every world entity, so it has to run from
        // an exclusive system in `First`, after the previous frame's commands
        // were applied.
        app.add_systems(First, process_change_level);
    }
}

/// Areas of the current world that switch worlds when the player enters them.
#[derive(Resource, Default)]
pub struct ChangeLevelTriggers(pub Vec<ChangeLevelTrigger>);

/// Level change requested by the previous frame, consumed in `First`.
#[derive(Resource, Default)]
struct PendingChangeLevel(Option<(String, String)>);

fn detect_change_level(
    players: Query<&Transform, With<PlayerCharacter>>,
    mut triggers: ResMut<ChangeLevelTriggers>,
    mut pending: ResMut<PendingChangeLevel>,
) {
    let _perf = perf_debug!();
    if pending.0.is_some() {
        return;
    }
    let Some(position) = players.iter().next().map(|transform| transform.translation) else {
        return;
    };
    let Some(index) = triggers
        .0
        .iter()
        .position(|trigger| trigger.contains(position))
    else {
        return;
    };
    let trigger = triggers.0.remove(index);
    info!(
        trigger = %trigger.name,
        level = %trigger.level,
        start_vob = %trigger.start_vob,
        "Change level triggered"
    );
    pending.0 = Some((trigger.level, trigger.start_vob));
}

fn process_change_level(world: &mut World) {
    let _perf = perf_debug!();
    let Some((level, start_vob)) = world.resource_mut::<PendingChangeLevel>().0.take() else {
        return;
    };
    begin_level_change(world, &level, &start_vob);
}
