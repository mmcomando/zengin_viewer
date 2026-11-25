//! Manual per-npc trace: animations, position, weapons and actions of the npc
//! selected by [`DebugNpc`].

use bevy::asset::AssetPath;
use bevy::prelude::*;

use crate::graphics::AnimationController;
use crate::perf_debug;
use crate::test_log;
use crate::zengin::GameNpc;
use crate::zengin::npc_actions::{
    AttackAction, GoToNpcAction, GoToWaypointAction, NpcActionQueue, StandUpAction,
    UseItemStateAction, UseMobAction,
};
use crate::zengin::npc_routines::{NpcEquippedWeapon, NpcWeaponMode};
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::manager::ScriptManager;
use crate::zengin::script::state::{NpcWeaponsUnequipped, WeaponMode};
use crate::zengin::script::symbol_id::InstanceId;

/// Adds the [`DebugNpc`] trace systems.
pub struct DebugNpcPlugin;

impl Plugin for DebugNpcPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugNpc>();
        app.add_message::<MobUseTrace>();
        app.add_message::<MobUseReleaseTrace>();
        app.add_message::<DamageTrace>();
        app.add_message::<DeathTrace>();
        app.add_message::<ActionCancelTrace>();
        app.add_systems(
            Update,
            (
                mark_debug_npc,
                log_animation_changes,
                log_action_changes,
                log_weapon_changes,
                log_periodic_state,
            )
                .run_if(debug_npc_configured),
        );
        // The event consumers run even without a filter so their readers stay
        // in sync with the message buffers.
        app.add_systems(
            Update,
            (
                log_mob_use_messages,
                log_mob_release_messages,
                log_damage_messages,
                log_death_messages,
                log_action_cancel_messages,
                log_weapons_unequipped_messages,
            ),
        );
    }
}

/// Case-insensitive substrings of script symbol names of the traced npcs. An
/// empty list disables the trace.
#[derive(Resource, Default)]
pub(crate) struct DebugNpc(Vec<String>);

impl DebugNpc {
    /// Replaces the traced npcs with the comma separated `text`; blank parts
    /// are ignored and an empty `text` disables the trace.
    pub(crate) fn set(&mut self, text: &str) {
        self.0 = split_filters(text);
    }

    /// Adds the comma separated npcs in `text`; `clear` drops the filters added
    /// before.
    pub(crate) fn add(&mut self, text: &str, clear: bool) {
        if clear {
            self.0.clear();
        }
        self.0.extend(split_filters(text));
    }

    /// Current filters joined with `", "`.
    pub(crate) fn text(&self) -> String {
        self.0.join(", ")
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn matches(&self, name: &str) -> bool {
        if self.0.iter().any(|filter| filter == "*") {
            return true;
        }
        let name = name.to_lowercase();
        self.0.iter().any(|filter| name.contains(filter.as_str()))
    }
}

fn split_filters(text: &str) -> Vec<String> {
    text.split(',')
        .map(|part| part.trim().to_lowercase())
        .filter(|part| !part.is_empty())
        .collect()
}

/// One mob use, logged by the trace when the actor is traced.
#[derive(Message)]
pub(crate) struct MobUseTrace {
    pub(crate) actor: InstanceId,
    pub(crate) is_player: bool,
    pub(crate) mob: Entity,
    pub(crate) vob_name: String,
    pub(crate) visual_stem: String,
    pub(crate) focus_name: String,
    pub(crate) condition_func: String,
    pub(crate) on_state_func: String,
    pub(crate) scheme: String,
    pub(crate) state: i32,
    pub(crate) animation: String,
    pub(crate) transition: Option<String>,
}

/// A mob use that ended and why, logged by the trace when the actor is traced.
#[derive(Message)]
pub(crate) struct MobUseReleaseTrace {
    pub(crate) actor: InstanceId,
    pub(crate) mob: Entity,
    pub(crate) reason: MobUseReleaseReason,
    pub(crate) transition: Option<String>,
    /// Pose the auto-end plays, if any.
    pub(crate) animation: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum MobUseReleaseReason {
    StopRequest,
    AutoEnd,
    LeavesChest,
    Released,
}

/// A confirmed damage hit, logged by the trace when either side is traced.
#[derive(Message)]
pub(crate) struct DamageTrace {
    pub(crate) shooter: InstanceId,
    pub(crate) target: InstanceId,
    pub(crate) shooter_is_player: bool,
    pub(crate) target_is_player: bool,
    pub(crate) damage: i32,
    pub(crate) hitpoints: i32,
}

/// A dying npc that ran its script death state.
#[derive(Message)]
pub(crate) struct DeathTrace {
    pub(crate) npc: InstanceId,
    pub(crate) killer: Option<InstanceId>,
}

/// An executing npc action canceled by `Npc_ClearAIQueue`.
#[derive(Message)]
pub(crate) struct ActionCancelTrace {
    pub(crate) npc: InstanceId,
    pub(crate) entity: Entity,
    pub(crate) goto: Option<String>,
    pub(crate) name: String,
}

/// Trace state of a matched npc, its presence marks the traced entity.
#[derive(Component)]
struct DebugNpcTrace {
    animation: String,
    actions: String,
    weapon: String,
    timer: Timer,
}

impl Default for DebugNpcTrace {
    fn default() -> Self {
        Self {
            animation: String::new(),
            actions: String::new(),
            weapon: String::new(),
            timer: Timer::from_seconds(0.5, TimerMode::Repeating),
        }
    }
}

fn debug_npc_configured(debug_npc: Res<DebugNpc>) -> bool {
    !debug_npc.is_empty()
}

fn mark_debug_npc(
    mut commands: Commands,
    scripts: Option<Res<ScriptResource>>,
    debug_npc: Res<DebugNpc>,
    npcs: Query<(Entity, &GameNpc, Has<DebugNpcTrace>)>,
    spawned: Query<Entity, Added<GameNpc>>,
) {
    let _perf = perf_debug!();
    if debug_npc.is_empty() {
        return;
    }
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
        let matches = name.as_deref().is_some_and(|name| debug_npc.matches(name));
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
        test_log!("DebugNPC: animation entity({entity}) {signature}");
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
        test_log!("DebugNPC: actions {signature}");
        trace.actions = signature;
    }
}

fn log_weapon_changes(mut npcs: Query<(Entity, &mut DebugNpcTrace, Option<&NpcWeaponMode>)>) {
    let _perf = perf_debug!();
    for (entity, mut trace, mode) in &mut npcs {
        let Some(mode) = mode else {
            continue;
        };
        let current = format!("{:?}", mode.0);
        if trace.weapon.is_empty() {
            trace.weapon = current;
            continue;
        }
        if trace.weapon == current {
            continue;
        }
        if mode.0 == WeaponMode::Fists {
            test_log!(
                "DebugNPC: weapon entity({entity}) sheathed its {} weapon",
                trace.weapon
            );
        } else {
            test_log!("DebugNPC: weapon entity({entity}) drew its {current} weapon");
        }
        trace.weapon = current;
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
        test_log!(
            "DebugNPC: state position=({:.2}, {:.2}, {:.2}) weapon_mode={weapon_mode} melee={melee} ranged={ranged}",
            position.x,
            position.y,
            position.z
        );
    }
}

fn is_traced(traced: &Query<&GameNpc, With<DebugNpcTrace>>, instances: &[InstanceId]) -> bool {
    traced
        .iter()
        .any(|npc| instances.contains(&npc.instance_id()))
}

fn log_mob_use_messages(
    mut messages: MessageReader<MobUseTrace>,
    traced: Query<&GameNpc, With<DebugNpcTrace>>,
) {
    for event in messages.read() {
        if !is_traced(&traced, &[event.actor]) {
            continue;
        }
        let actor = if event.is_player { "Player" } else { "npc" };
        test_log!(
            "DebugNPC: mob entity({}) {actor}({}) uses mob({}) vob({}) scheme({}) state({}) animation({}) transition({:?}) visual_stem({}) focus({}) condition_func({}) on_state_func({}) -> state({})",
            event.mob,
            event.actor,
            event.mob,
            event.vob_name,
            event.scheme,
            event.state,
            event.animation,
            event.transition,
            event.visual_stem,
            event.focus_name,
            event.condition_func,
            event.on_state_func,
            event.state
        );
    }
}

fn log_mob_release_messages(
    mut messages: MessageReader<MobUseReleaseTrace>,
    traced: Query<&GameNpc, With<DebugNpcTrace>>,
) {
    for event in messages.read() {
        if !is_traced(&traced, &[event.actor]) {
            continue;
        }
        match event.reason {
            MobUseReleaseReason::StopRequest => test_log!(
                "DebugNPC: release npc({}) released mob({}) on stop request",
                event.actor,
                event.mob
            ),
            MobUseReleaseReason::AutoEnd => test_log!(
                "DebugNPC: release Player({}) auto-ends mob({}) use, pose({})",
                event.actor,
                event.mob,
                event.animation.as_deref().unwrap_or("-")
            ),
            MobUseReleaseReason::LeavesChest => test_log!(
                "DebugNPC: release Player({}) leaves chest({}) transition({:?})",
                event.actor,
                event.mob,
                event.transition
            ),
            MobUseReleaseReason::Released => test_log!(
                "DebugNPC: release Player({}) released mob({})",
                event.actor,
                event.mob
            ),
        }
    }
}

fn log_damage_messages(
    mut messages: MessageReader<DamageTrace>,
    traced: Query<&GameNpc, With<DebugNpcTrace>>,
) {
    for event in messages.read() {
        if !is_traced(&traced, &[event.shooter, event.target]) {
            continue;
        }
        if event.target_is_player {
            test_log!(
                "DebugNPC: damage Npc({}) dealt {} damage to player({}), hitpoints now {}",
                event.shooter,
                event.damage,
                event.target,
                event.hitpoints
            );
        } else if event.shooter_is_player {
            test_log!(
                "DebugNPC: damage Player({}) dealt {} damage to npc({}), hitpoints now {}",
                event.shooter,
                event.damage,
                event.target,
                event.hitpoints
            );
        } else {
            test_log!(
                "DebugNPC: damage Npc({}) dealt {} damage to npc({}), hitpoints now {}",
                event.shooter,
                event.damage,
                event.target,
                event.hitpoints
            );
        }
    }
}

fn log_death_messages(
    mut messages: MessageReader<DeathTrace>,
    traced: Query<&GameNpc, With<DebugNpcTrace>>,
) {
    for event in messages.read() {
        if !is_traced(&traced, &[event.npc]) {
            continue;
        }
        let killer = event
            .killer
            .map_or_else(|| "none".to_string(), |killer| killer.value().to_string());
        test_log!(
            "DebugNPC: death Npc({}) ran the ZS_DEAD state for killer({killer})",
            event.npc
        );
    }
}

fn log_weapons_unequipped_messages(
    mut messages: MessageReader<NpcWeaponsUnequipped>,
    traced: Query<&GameNpc, With<DebugNpcTrace>>,
) {
    for event in messages.read() {
        if !is_traced(&traced, &[event.instance_id]) {
            continue;
        }
        test_log!(
            "DebugNPC: weapon npc({}) unequipped its weapons",
            event.instance_id
        );
    }
}

fn log_action_cancel_messages(
    mut messages: MessageReader<ActionCancelTrace>,
    traced: Query<&GameNpc, With<DebugNpcTrace>>,
) {
    for event in messages.read() {
        if !is_traced(&traced, &[event.npc]) {
            continue;
        }
        let name = &event.name;
        let entity = event.entity;
        match &event.goto {
            Some(goto) => test_log!(
                "DebugNPC: cancel Npc_ClearAIQueue npc({}) canceled the executing goto({goto}) entity({entity}) name({name})",
                event.npc
            ),
            None => test_log!(
                "DebugNPC: cancel Npc_ClearAIQueue npc({}) canceled the executing action entity({entity}) name({name})",
                event.npc
            ),
        }
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
