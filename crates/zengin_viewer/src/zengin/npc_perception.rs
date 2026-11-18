//! Engine side of npc perception: periodic scans that hand sensible enemies to
//! the script perception callbacks registered with `Npc_PercEnable`.
//!
//! Only npcs with a visual (`AnimationController`, i.e. near the player) are
//! scanned, matching the engine's `AiNormal` process policy.

use bevy::prelude::*;

use crate::game_state::AppState;
use crate::graphics::AnimationController;
use crate::zengin::GameNpc;
use crate::zengin::script::bridge::ScriptChangesApplied;
use crate::zengin::script::component::ScriptResource;

pub(crate) struct NpcPerceptionPlugin;

impl Plugin for NpcPerceptionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                seed_npc_perception_timers,
                run_npc_perceptions.after(ScriptChangesApplied),
            )
                .chain()
                .run_if(in_state(AppState::InGame)),
        );
    }
}

/// Countdown to the next perception scan of an npc.
#[derive(Component, Debug, Clone)]
pub(crate) struct NpcPerceptionTimer {
    next_in: f32,
}

impl Default for NpcPerceptionTimer {
    fn default() -> Self {
        Self {
            next_in: INITIAL_PERCEPTION_DELAY_SECS,
        }
    }
}

/// First scan happens a moment after the visual appears, so the routine state
/// start hook had time to register its perception callbacks.
const INITIAL_PERCEPTION_DELAY_SECS: f32 = 1.0;
/// Lower bound of the scan cadence, protects against a zero `Npc_SetPercTime`.
const MIN_PERCEPTION_INTERVAL_SECS: f32 = 0.1;

#[allow(clippy::type_complexity)]
fn seed_npc_perception_timers(
    mut commands: Commands,
    npcs: Query<Entity, (Added<AnimationController>, With<GameNpc>, Without<ChildOf>)>,
) {
    for entity in &npcs {
        commands
            .entity(entity)
            .insert(NpcPerceptionTimer::default());
    }
}

#[allow(clippy::type_complexity)]
fn run_npc_perceptions(
    time: Res<Time>,
    gothic_script: Res<ScriptResource>,
    mut npcs: Query<
        (Entity, &GameNpc, &mut NpcPerceptionTimer),
        (With<AnimationController>, Without<ChildOf>),
    >,
) {
    if npcs.is_empty() {
        return;
    }
    let delta = time.delta_secs();
    let mut due = Vec::new();
    for (entity, npc, mut timer) in &mut npcs {
        timer.next_in -= delta;
        if timer.next_in > 0.0 {
            continue;
        }
        due.push((entity, npc.instance_id()));
    }
    if due.is_empty() {
        return;
    }

    let mut script = gothic_script.lock();
    for (entity, instance) in due {
        let Ok((_, _, mut timer)) = npcs.get_mut(entity) else {
            continue;
        };
        let interval = script.perception_interval(instance);
        timer.next_in = interval.max(MIN_PERCEPTION_INTERVAL_SECS);
        script.run_perception(instance);
    }
}
