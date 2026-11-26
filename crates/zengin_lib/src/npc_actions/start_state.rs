use bevy::prelude::*;

use crate::GameNpc;
use crate::npc_actions::action::ExecutingActionMarker;
use crate::npc_routines::{RoutineExecutor, start_ai_state};
use crate::script::component::ScriptResource;
use crate::script::state::{AttackChange, ScriptMessages};
use crate::script::symbol_id::{FunctionId, InstanceId};
use dev_util::perf_debug;

/// Starts an `AI_StartState` request; the running routine or AI state is ended
/// and the requested one takes over.
#[derive(Component)]
#[require(ExecutingActionMarker)]
pub struct StartStateAction {
    pub state: FunctionId,
    pub other: Option<InstanceId>,
}

pub fn start_start_state_action(
    mut commands: Commands,
    gothic_script: Res<ScriptResource>,
    messages: Res<ScriptMessages>,
    mut npcs: Query<
        (
            Entity,
            &GameNpc,
            &StartStateAction,
            Option<&mut RoutineExecutor>,
        ),
        Added<StartStateAction>,
    >,
) {
    let _perf = perf_debug!();
    let mut script = gothic_script.lock();
    for (entity, npc, action, mut executor) in &mut npcs {
        let instance = npc.instance_id();
        if script.npc_state(instance).is_some_and(|npc| npc.live.dead) {
            commands.entity(entity).remove::<ExecutingActionMarker>();
            continue;
        }
        if let Some(executor) = start_ai_state(
            &mut script,
            instance,
            executor.as_deref_mut(),
            action.state,
            action.other,
        ) {
            commands.entity(entity).insert(executor);
        }
        // `zs_attack_loop` is the retail trigger for the fight, but it can end
        // before reaching `AI_Attack` when the ai variables are not initialised,
        // so starting the state also requests the attack. The request lands
        // behind the actions the start hook queued.
        if let Some(target) = action.other
            && script
                .call_get_name(action.state.symbol())
                .eq_ignore_ascii_case("zs_attack")
        {
            messages.attack.send(AttackChange {
                instance_id: instance,
                target,
            });
        }
    }
}

pub fn run_start_state_action(
    mut commands: Commands,
    actions: Query<(Entity, Ref<StartStateAction>)>,
) {
    let _perf = perf_debug!();
    for (entity, action) in &actions {
        // The start owns one update before the queue continues, so the actions
        // the start hook queued are enqueued before the state loop can run.
        if action.is_added() {
            continue;
        }
        commands.entity(entity).remove::<ExecutingActionMarker>();
    }
}
