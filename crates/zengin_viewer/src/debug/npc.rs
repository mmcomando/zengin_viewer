//! Manual per-npc trace: animations, position, weapons and actions of the npc
//! selected by [`DebugNpc`].

use std::sync::PoisonError;

use bevy::asset::AssetPath;
use bevy::prelude::*;

use crate::zengin::AnimationController;
use crate::zengin::PlayerCharacter;
use crate::zengin::{
    AttackAction, ClearAiQueueChange, GameMob, GameNpc, GoToNpcAction, GoToWaypointAction,
    InstanceId, MobBase, MobPose, MobUsePose, NpcActionQueue, NpcDied, NpcEntitiesChanged,
    NpcEquippedWeapon, NpcHealth, NpcMobUse, NpcWeaponMode, ScriptManager, ScriptResource,
    StandUpAction, UseItemStateAction, UseMobAction, WeaponMode,
};
use dev_util::perf_debug;
use dev_util::test_log;

/// Adds the [`DebugNpc`] trace systems.
pub(crate) struct DebugNpcPlugin;

impl Plugin for DebugNpcPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugNpc>();
        app.add_systems(
            Update,
            (
                mark_debug_npc,
                log_animation_changes,
                log_action_changes,
                log_mob_use_changes,
                log_mob_release_changes,
                log_health_changes,
                log_periodic_state,
            )
                .run_if(debug_npc_configured),
        );
        // The message consumers run even without a filter so their readers stay
        // in sync with the message buffers.
        app.add_systems(
            Update,
            (
                log_weapon_changes,
                log_death_messages,
                log_action_cancel_messages,
            ),
        );
    }
}

/// Case-insensitive substrings of script symbol names of the traced npcs. An
/// empty list disables the trace; `*` traces every npc.
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

/// Mob of the last logged use, so a removal can be reported with its id.
#[derive(Clone, Copy)]
struct MobUseSnapshot {
    mob: Entity,
    is_player: bool,
}

/// Trace state of a matched npc, its presence marks the traced entity.
#[derive(Component)]
struct DebugNpcTrace {
    animation: String,
    actions: String,
    timer: Timer,
    mob_use: Option<MobUseSnapshot>,
    health: Option<i32>,
    weapon_seen: bool,
    weapon_mode: Option<WeaponMode>,
    weapon_items: (Option<InstanceId>, Option<InstanceId>),
}

impl Default for DebugNpcTrace {
    fn default() -> Self {
        Self {
            animation: String::new(),
            actions: String::new(),
            timer: Timer::from_seconds(0.5, TimerMode::Repeating),
            mob_use: None,
            health: None,
            weapon_seen: false,
            weapon_mode: None,
            weapon_items: (None, None),
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

#[allow(clippy::type_complexity)]
fn log_mob_use_changes(
    mut npcs: Query<
        (
            Entity,
            &GameNpc,
            &NpcMobUse,
            &mut DebugNpcTrace,
            Has<PlayerCharacter>,
            Option<&MobUsePose>,
            Option<&AnimationController>,
        ),
        Added<NpcMobUse>,
    >,
    mobs: Query<(&MobBase, &MobPose, Option<&GameMob>)>,
) {
    let _perf = perf_debug!();
    for (_entity, npc, mob_use, mut trace, is_player, pose, controller) in &mut npcs {
        let Ok((base, mob_pose, game_mob)) = mobs.get(mob_use.mob) else {
            continue;
        };
        let animation = pose.map_or_else(
            || controller.map_or_else(|| "-".to_string(), |c| animation_label(c.current().path())),
            |pose| pose.pose().to_string(),
        );
        let transition = pose.map(MobUsePose::transition);
        let actor = if is_player { "Player" } else { "npc" };
        test_log!(
            "DebugNPC: mob entity({}) {actor}({}) uses mob({}) vob({}) scheme({}) state({}) animation({animation}) transition({transition:?}) visual_stem({}) focus({}) condition_func({}) on_state_func({}) -> state({})",
            mob_use.mob,
            npc.instance_id(),
            mob_use.mob,
            base.vob_name,
            mob_use.scheme,
            mob_use.state,
            mob_pose.visual_stem,
            base.focus_name,
            game_mob.map_or("", |mob| mob.condition_func.as_str()),
            game_mob.map_or("", |mob| mob.on_state_func.as_str()),
            mob_use.state
        );
        trace.mob_use = Some(MobUseSnapshot {
            mob: mob_use.mob,
            is_player,
        });
    }
}

fn log_mob_release_changes(
    mut removed: RemovedComponents<NpcMobUse>,
    mut npcs: Query<(Entity, &GameNpc, &mut DebugNpcTrace)>,
) {
    let _perf = perf_debug!();
    for entity in removed.read() {
        let Ok((_entity, npc, mut trace)) = npcs.get_mut(entity) else {
            continue;
        };
        let Some(snapshot) = trace.mob_use.take() else {
            continue;
        };
        let actor = if snapshot.is_player { "Player" } else { "npc" };
        test_log!(
            "DebugNPC: release {actor}({}) released mob({})",
            npc.instance_id(),
            snapshot.mob
        );
    }
}

fn log_health_changes(mut npcs: Query<(Entity, &mut DebugNpcTrace, Option<&NpcHealth>)>) {
    let _perf = perf_debug!();
    for (entity, mut trace, health) in &mut npcs {
        let Some(health) = health else {
            continue;
        };
        if trace.health == Some(health.hitpoints) {
            continue;
        }
        let first = trace.health.is_none();
        trace.health = Some(health.hitpoints);
        if first {
            continue;
        }
        test_log!(
            "DebugNPC: health entity({entity}) hitpoints now {}",
            health.hitpoints
        );
    }
}

fn log_weapon_changes(
    mut changes: MessageReader<NpcEntitiesChanged>,
    mut npcs: Query<(Entity, &mut DebugNpcTrace)>,
) {
    for change in changes.read() {
        let (mode, items) = {
            let runtime = change.handle.runtime();
            let state = runtime.read().unwrap_or_else(PoisonError::into_inner);
            (
                state.equipment.weapon_mode,
                (state.equipment.melee, state.equipment.ranged),
            )
        };
        for &entity in &change.entities {
            let Ok((entity, mut trace)) = npcs.get_mut(entity) else {
                continue;
            };
            if !trace.weapon_seen {
                trace.weapon_seen = true;
                trace.weapon_mode = mode;
                trace.weapon_items = items;
                continue;
            }
            let previous_mode = trace.weapon_mode;
            let previous_items = trace.weapon_items;
            if previous_mode == mode && previous_items == items {
                continue;
            }
            trace.weapon_mode = mode;
            trace.weapon_items = items;
            let unequipped = previous_items != (None, None) && items == (None, None);
            if unequipped {
                test_log!("DebugNPC: weapon entity({entity}) unequipped its weapons");
            } else if matches!(mode, None | Some(WeaponMode::Fists)) {
                let previous =
                    previous_mode.map_or_else(|| "none".to_string(), |mode| format!("{mode:?}"));
                test_log!("DebugNPC: weapon entity({entity}) sheathed its {previous} weapon");
            } else if mode != previous_mode {
                let mode = mode.expect("checked above");
                test_log!("DebugNPC: weapon entity({entity}) drew its {mode:?} weapon");
            }
        }
    }
}

fn log_death_messages(
    mut messages: MessageReader<NpcDied>,
    traced: Query<&GameNpc, With<DebugNpcTrace>>,
) {
    for event in messages.read() {
        if !is_traced(&traced, &[event.instance_id]) {
            continue;
        }
        let killer = event
            .killer
            .map_or_else(|| "none".to_string(), |killer| killer.value().to_string());
        test_log!(
            "DebugNPC: death Npc({}) killer({killer})",
            event.instance_id
        );
    }
}

fn log_action_cancel_messages(
    mut messages: MessageReader<ClearAiQueueChange>,
    traced: Query<&GameNpc, With<DebugNpcTrace>>,
) {
    for event in messages.read() {
        if !is_traced(&traced, &[event.instance_id]) {
            continue;
        }
        test_log!("DebugNPC: clear queue npc({})", event.instance_id);
    }
}

fn is_traced(traced: &Query<&GameNpc, With<DebugNpcTrace>>, instances: &[InstanceId]) -> bool {
    traced
        .iter()
        .any(|npc| instances.contains(&npc.instance_id()))
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
