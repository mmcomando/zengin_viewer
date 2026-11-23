//! Manual per-npc trace: animations, position, weapons and actions of the npc
//! selected by [`DebugNpc`].

use bevy::asset::AssetPath;
use bevy::prelude::*;

use crate::graphics::AnimationController;
use crate::perf_debug;
use crate::zengin::GameNpc;
use crate::zengin::npc_actions::{
    AttackAction, GoToNpcAction, GoToWaypointAction, NpcActionQueue, StandUpAction,
    UseItemStateAction, UseMobAction,
};
use crate::zengin::npc_routines::{NpcEquippedWeapon, NpcWeaponMode};
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::manager::ScriptManager;
use crate::zengin::script::symbol_id::InstanceId;

/// Adds the [`DebugNpc`] trace systems.
pub struct DebugNpcPlugin;

impl Plugin for DebugNpcPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugNpc>();
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

/// Case-insensitive substring of the script symbol name of the npc to trace.
/// `None` disables the trace.
#[derive(Resource, Default)]
pub(crate) struct DebugNpc(Option<String>);

impl DebugNpc {
    /// Selects the traced npc; a blank `name` disables the trace.
    pub(crate) fn set(&mut self, name: &str) {
        let name = name.trim().to_lowercase();
        self.0 = (!name.is_empty()).then_some(name);
    }

    /// Current filter as entered, blank when the trace is disabled.
    pub(crate) fn text(&self) -> &str {
        self.0.as_deref().unwrap_or_default()
    }

    fn filter(&self) -> Option<&str> {
        self.0.as_deref()
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

fn debug_npc_configured(debug_npc: Res<DebugNpc>) -> bool {
    debug_npc.filter().is_some()
}

fn mark_debug_npc(
    mut commands: Commands,
    scripts: Option<Res<ScriptResource>>,
    debug_npc: Res<DebugNpc>,
    npcs: Query<(Entity, &GameNpc, Has<DebugNpcTrace>)>,
    spawned: Query<Entity, Added<GameNpc>>,
) {
    let _perf = perf_debug!();
    let Some(filter) = debug_npc.filter() else {
        return;
    };
    let Some(scripts) = scripts else {
        return;
    };
    // A changed filter has to be re-evaluated for npcs marked by the old one,
    // otherwise only newly spawned npcs need to be considered.
    let rescan = debug_npc.is_changed();
    let scripts = scripts.lock();
    for (entity, npc, traced) in &npcs {
        if !rescan && (traced || !spawned.contains(entity)) {
            continue;
        }
        let name = scripts
            .symbol_table()
            .symbol_name(npc.instance_id().value());
        let matches = name
            .as_deref()
            .is_some_and(|name| name.to_lowercase().contains(filter));
        match (matches, traced) {
            (true, false) => {
                info!(
                    "DebugNPC: tracing npc({}) instance({}) entity({entity})",
                    name.as_deref().unwrap_or_default(),
                    npc.instance_id()
                );
                commands.entity(entity).insert(DebugNpcTrace::default());
            }
            (false, true) => {
                info!(
                    "DebugNPC: stopped tracing entity({entity}) instance({})",
                    npc.instance_id()
                );
                commands.entity(entity).remove::<DebugNpcTrace>();
            }
            _ => {}
        }
    }
}

fn log_animation_changes(mut npcs: Query<(Entity, &AnimationController, &mut DebugNpcTrace)>) {
    let _perf = perf_debug!();
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
        Option<&GoToNpcAction>,
        Option<&UseMobAction>,
        Option<&UseItemStateAction>,
        Option<&StandUpAction>,
        Option<&NpcActionQueue>,
    )>,
) {
    let _perf = perf_debug!();
    for (mut trace, attack, go_to, go_to_npc, use_mob, use_item, stand_up, queue) in &mut npcs {
        let mut actions: Vec<String> = Vec::new();
        if let Some(action) = attack {
            actions.push(format!("Attack(target={})", action.target));
        }
        if let Some(action) = go_to {
            actions.push(format!("GoTo({})", action.waypoint.as_str()));
        }
        if let Some(action) = go_to_npc {
            actions.push(format!("GoToNpc(target={})", action.target));
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
    let _perf = perf_debug!();
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
