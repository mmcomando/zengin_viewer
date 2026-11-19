//! Engine side of npc perception: periodic scans that hand sensible enemies to
//! the script perception callbacks registered with `Npc_PercEnable`.
//!
//! Only npcs with a visual (`AnimationController`, i.e. near the player) are
//! scanned, matching the engine's `AiNormal` process policy.

use std::time::{Duration, Instant};

use bevy::prelude::*;

use crate::game_state::AppState;
use crate::graphics::AnimationController;
use crate::updater::{Periodic, PeriodicAppExt, PeriodicKind, PeriodicUpdate};
use crate::zengin::GameNpc;
use crate::zengin::script::bridge::ScriptChangesApplied;
use crate::zengin::script::component::ScriptResource;

pub(crate) struct NpcPerceptionPlugin;

impl Plugin for NpcPerceptionPlugin {
    fn build(&self, app: &mut App) {
        app.add_periodic_updates::<NpcPerceptionKind>();
        app.add_systems(
            Update,
            (
                seed_npc_perception_tasks,
                run_npc_perceptions.after(ScriptChangesApplied),
            )
                .chain()
                .run_if(in_state(AppState::InGame)),
        );
    }
}

/// Selects the npc perception update stream.
struct NpcPerceptionKind;

impl PeriodicKind for NpcPerceptionKind {}

/// First scan happens a moment after the visual appears, so the routine state
/// start hook had time to register its perception callbacks.
const INITIAL_PERCEPTION_INTERVAL: Duration = Duration::from_secs(1);
/// Lower bound of the scan cadence, protects against a zero `Npc_SetPercTime`.
const MIN_PERCEPTION_INTERVAL_SECS: f32 = 0.1;

#[allow(clippy::type_complexity)]
fn seed_npc_perception_tasks(
    mut commands: Commands,
    npcs: Query<Entity, (Added<AnimationController>, With<GameNpc>, Without<ChildOf>)>,
) {
    for entity in &npcs {
        commands
            .entity(entity)
            .insert(Periodic::<NpcPerceptionKind>::new(
                INITIAL_PERCEPTION_INTERVAL,
            ));
    }
}

fn run_npc_perceptions(
    mut updates: MessageReader<PeriodicUpdate<NpcPerceptionKind>>,
    gothic_script: Res<ScriptResource>,
    npcs: Query<&GameNpc, (With<AnimationController>, Without<ChildOf>)>,
) {
    if updates.is_empty() {
        return;
    }
    let mut script = gothic_script.lock();
    for update in updates.read() {
        let start = Instant::now();
        let Ok(npc) = npcs.get(update.entity) else {
            continue;
        };
        let instance = npc.instance_id();
        let interval = script.perception_interval(instance);
        // Applied to the reschedule after the next tick.
        update.control.set_period(Duration::from_secs_f32(
            interval.max(MIN_PERCEPTION_INTERVAL_SECS),
        ));
        script.run_perception(instance);
        update
            .control
            .update_effort(start.elapsed().as_micros() as u64);
    }
}
