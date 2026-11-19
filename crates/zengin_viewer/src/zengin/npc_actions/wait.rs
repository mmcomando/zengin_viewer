use bevy::prelude::*;

use crate::zengin::npc_actions::action::ExecutingActionMarker;

/// Idles the npc for a fixed time (`AI_WaitMS`), blocking later queued actions.
#[derive(Component)]
#[require(ExecutingActionMarker)]
pub(crate) struct WaitAction {
    /// Remaining time in seconds.
    pub(crate) remaining: f32,
}

pub(crate) fn run_wait_action(
    time: Res<Time>,
    mut commands: Commands,
    mut npcs: Query<(Entity, &mut WaitAction)>,
) {
    let delta = time.delta_secs();
    for (entity, mut action) in &mut npcs {
        action.remaining -= delta;
        if action.remaining > 0.0 {
            continue;
        }
        debug!("npc({entity}) finished action wait");
        commands.entity(entity).remove::<ExecutingActionMarker>();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.add_systems(Update, run_wait_action);
        app
    }

    #[test]
    fn wait_action_runs_out_after_its_duration() {
        let mut app = app();
        let entity = app
            .world_mut()
            .spawn((WaitAction { remaining: 0.5 }, ExecutingActionMarker))
            .id();

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(250));
        app.update();
        assert!(app.world().get::<ExecutingActionMarker>(entity).is_some());

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(300));
        app.update();
        assert!(app.world().get::<ExecutingActionMarker>(entity).is_none());
    }
}
