//! Manual per-npc trace: animations, position, weapons and actions of the npc
//! selected by [`DEBUG_NPC`].

use bevy::asset::AssetPath;
use bevy::prelude::*;

use crate::graphics::AnimationController;
use crate::zengin::GameNpc;
use crate::zengin::npc_actions::{
    AttackAction, GoToWaypointAction, NpcActionQueue, StandUpAction, UseItemStateAction,
    UseMobAction,
};
use crate::zengin::npc_routines::{NpcEquippedWeapon, NpcWeaponMode};
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::manager::ScriptManager;
use crate::zengin::script::symbol_id::InstanceId;

/// Case-insensitive substring of the script symbol name of the npc to trace.
/// Set it by hand when debugging, `None` disables the trace.
// pub(crate) static DEBUG_NPC: Option<&'static str> = Some("lares");
pub(crate) static DEBUG_NPC: Option<&'static str> = None;

/// Adds the [`DEBUG_NPC`] trace systems.
pub struct DebugNpcPlugin;

impl Plugin for DebugNpcPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                mark_debug_npc,
                log_animation_changes,
                log_action_changes,
                log_periodic_state,
            )
                .run_if(debug_npc_configured),
        );
    }
}

/// Trace state of a matched npc, its presence marks the traced entity.
#[derive(Component)]
struct DebugNpcTrace {
    animation: String,
    actions: String,
    timer: Timer,
}

impl Default for DebugNpcTrace {
    fn default() -> Self {
        Self {
            animation: String::new(),
            actions: String::new(),
            timer: Timer::from_seconds(0.5, TimerMode::Repeating),
        }
    }
}

fn debug_npc_configured() -> bool {
    DEBUG_NPC.is_some_and(|filter| !filter.trim().is_empty())
}

fn debug_npc_filter() -> Option<String> {
    let filter = DEBUG_NPC?.trim().to_lowercase();
    (!filter.is_empty()).then_some(filter)
}

#[allow(clippy::type_complexity)]
fn mark_debug_npc(
    mut commands: Commands,
    scripts: Option<Res<ScriptResource>>,
    npcs: Query<(Entity, &GameNpc), (Added<GameNpc>, Without<DebugNpcTrace>)>,
) {
    let Some(filter) = debug_npc_filter() else {
        return;
    };
    let Some(scripts) = scripts else {
        return;
    };
    let scripts = scripts.lock();
    for (entity, npc) in &npcs {
        let Some(name) = scripts
            .symbol_table()
            .symbol_name(npc.instance_id().value())
        else {
            continue;
        };
        if !name.to_lowercase().contains(&filter) {
            continue;
        }
        info!(
            "DebugNPC: tracing npc({name}) instance({}) entity({entity})",
            npc.instance_id()
        );
        commands.entity(entity).insert(DebugNpcTrace::default());
    }
}

fn log_animation_changes(mut npcs: Query<(Entity, &AnimationController, &mut DebugNpcTrace)>) {
    for (entity, controller, mut trace) in &mut npcs {
        let current = animation_label(controller.current().path());
        let next = controller
            .next()
            .map_or_else(|| "-".to_string(), |next| animation_label(next.path()));
        let root_motion = controller.root_motion();
        let signature = format!("current={current} next={next} root_motion={root_motion}");
        if trace.animation == signature {
            continue;
        }
        info!("DebugNPC: animation entity({entity}) {signature}");
        trace.animation = signature;
    }
}

#[allow(clippy::type_complexity)]
fn log_action_changes(
    mut npcs: Query<(
        &mut DebugNpcTrace,
        Option<&AttackAction>,
        Option<&GoToWaypointAction>,
        Option<&UseMobAction>,
        Option<&UseItemStateAction>,
        Option<&StandUpAction>,
        Option<&NpcActionQueue>,
    )>,
) {
    for (mut trace, attack, go_to, use_mob, use_item, stand_up, queue) in &mut npcs {
        let mut actions: Vec<String> = Vec::new();
        if let Some(action) = attack {
            actions.push(format!("Attack(target={})", action.target));
        }
        if let Some(action) = go_to {
            actions.push(format!("GoTo({})", action.waypoint.as_str()));
        }
        if let Some(action) = use_mob {
            actions.push(format!(
                "UseMob({}, state={})",
                action.scheme.as_str(),
                action.state
            ));
        }
        if let Some(action) = use_item {
            actions.push(format!(
                "UseItemState({}, {}, state={})",
                action.item,
                action.scheme.as_str(),
                action.state
            ));
        }
        if stand_up.is_some() {
            actions.push("StandUp".to_string());
        }
        if let Some(queue) = queue
            && !queue.is_empty()
        {
            actions.push("queued".to_string());
        }
        let signature = if actions.is_empty() {
            "idle".to_string()
        } else {
            actions.join(", ")
        };
        if trace.actions == signature {
            continue;
        }
        info!("DebugNPC: actions {signature}");
        trace.actions = signature;
    }
}

#[allow(clippy::type_complexity)]
fn log_periodic_state(
    time: Res<Time>,
    scripts: Option<Res<ScriptResource>>,
    mut npcs: Query<(
        &GlobalTransform,
        &mut DebugNpcTrace,
        Option<&NpcEquippedWeapon>,
        Option<&NpcWeaponMode>,
    )>,
) {
    let Some(scripts) = scripts else {
        return;
    };
    for (transform, mut trace, weapon, mode) in &mut npcs {
        if !trace.timer.tick(time.delta()).just_finished() {
            continue;
        }
        let position = transform.translation();
        let weapon_mode = mode.map_or_else(|| "none".to_string(), |mode| format!("{:?}", mode.0));
        let mut script = scripts.lock();
        let melee = weapon
            .and_then(|weapon| weapon.melee)
            .map_or_else(|| "none".to_string(), |item| item_name(&mut script, item));
        let ranged = weapon
            .and_then(|weapon| weapon.ranged)
            .map_or_else(|| "none".to_string(), |item| item_name(&mut script, item));
        info!(
            "DebugNPC: state position=({:.2}, {:.2}, {:.2}) weapon_mode={weapon_mode} melee={melee} ranged={ranged}",
            position.x, position.y, position.z
        );
    }
}

fn animation_label(path: Option<&AssetPath>) -> String {
    let Some(path) = path else {
        return "-".to_string();
    };
    path.path().file_stem().map_or_else(
        || path.path().display().to_string(),
        |stem| stem.to_string_lossy().into_owned(),
    )
}

fn item_name(script: &mut ScriptManager, item: InstanceId) -> String {
    if let Some(name) = script.item_display_name(item) {
        return name;
    }
    script
        .symbol_table()
        .symbol_name(item.value())
        .unwrap_or_else(|| item.to_string())
}
