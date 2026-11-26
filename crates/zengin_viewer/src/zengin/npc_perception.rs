//! Engine side of npc perception: periodic scans that hand sensible enemies to
//! the script perception callbacks registered with `Npc_PercEnable`.
//!
//! Only npcs with a visual (`AnimationController`, i.e. near the player) are
//! scanned and get their sight cache refreshed, matching the engine's `AiNormal`
//! process policy.

use std::time::{Duration, Instant};

use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;

use crate::game_state::AppState;
use crate::graphics::AnimationController;
use crate::gui::{InventoryUiState, JournalUiState};
use crate::updater::{Periodic, PeriodicAppExt, PeriodicKind, PeriodicUpdate};
use crate::zengin::GameNpc;
use crate::zengin::STATIC_OBJECT;
use crate::zengin::dialog::DialogUiState;
use crate::zengin::script::bridge::ScriptChangesApplied;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::manager::ScriptManager;
use crate::zengin::script::state::PassivePerception;
use crate::zengin::script::symbol_id::InstanceId;
use dev_util::perf_debug;

pub(crate) struct NpcPerceptionPlugin;

impl Plugin for NpcPerceptionPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(NpcAutoTalkOptions { enabled: true });
        app.add_periodic_updates::<NpcPerceptionKind>();
        app.add_message::<VisibilityRefresh>();
        app.add_systems(
            Update,
            (
                refresh_visibility_on_events.after(ScriptChangesApplied),
                seed_npc_perception_tasks,
                dispatch_passive_perceptions.after(ScriptChangesApplied),
                run_npc_perceptions.after(ScriptChangesApplied),
            )
                .chain()
                .run_if(in_state(AppState::InGame)),
        );
    }
}

/// Requests a rebuild of every active observer's cached sight set, for events
/// that happen between perception scans.
#[derive(Message)]
pub(crate) struct VisibilityRefresh;

/// Runtime toggle of the `PERC_ASSESSPLAYER` dispatch, changed in the debug UI.
/// Disabled by default because npc started dialogs interrupt development.
#[derive(Resource, Default)]
pub(crate) struct NpcAutoTalkOptions {
    pub(crate) enabled: bool,
}

/// Selects the npc perception update stream.
struct NpcPerceptionKind;

impl PeriodicKind for NpcPerceptionKind {}

/// First scan happens a moment after the visual appears, so the routine state
/// start hook had time to register its perception callbacks.
const INITIAL_PERCEPTION_INTERVAL: Duration = Duration::from_secs(1);
/// Lower bound of the scan cadence, protects against a zero `Npc_SetPercTime`.
const MIN_PERCEPTION_INTERVAL_SECS: f32 = 0.1;

/// Height of the observer eye above the npc root (feet), in meters.
const NPC_EYE_HEIGHT: f32 = 1.6;
/// Height of the target body center above its root, in meters.
const NPC_BODY_CENTER_HEIGHT: f32 = 0.9;

/// Rebuilds the sight set of every active observer when an event between
/// perception scans can have changed what it sees. A perception broadcast is a
/// cue to re-check sight now.
fn refresh_visibility_on_events(
    mut requests: MessageReader<VisibilityRefresh>,
    mut broadcasts: MessageReader<PassivePerception>,
    gothic_script: Res<ScriptResource>,
    spatial_query: SpatialQuery,
    observers: Query<&GameNpc, (With<AnimationController>, Without<ChildOf>)>,
) {
    let _perf = perf_debug!();
    let invalidated = requests.read().count() > 0;
    let broadcast = broadcasts.read().count() > 0;
    if !invalidated && !broadcast {
        return;
    }
    let mut script = gothic_script.lock();
    for npc in &observers {
        refresh_visibility(&mut script, &spatial_query, npc.instance_id());
    }
}

#[allow(clippy::type_complexity)]
fn seed_npc_perception_tasks(
    mut commands: Commands,
    npcs: Query<Entity, (Added<AnimationController>, With<GameNpc>, Without<ChildOf>)>,
) {
    let _perf = perf_debug!();
    for entity in &npcs {
        commands
            .entity(entity)
            .insert(Periodic::<NpcPerceptionKind>::new(
                INITIAL_PERCEPTION_INTERVAL,
            ));
    }
}

/// Delivers `Npc_SendPassivePerc` broadcasts to the nearby npcs that have the
/// perception callback registered. Only animated (near player, `AiNormal`)
/// npcs receive broadcasts, matching the engine process policy.
fn dispatch_passive_perceptions(
    mut changes: MessageReader<PassivePerception>,
    gothic_script: Res<ScriptResource>,
    npcs: Query<&GameNpc, (With<AnimationController>, Without<ChildOf>)>,
) {
    let _perf = perf_debug!();
    if changes.is_empty() {
        return;
    }
    // Collect before locking, the reader borrow ends here.
    let changes: Vec<PassivePerception> = changes.read().copied().collect();
    let mut script = gothic_script.lock();
    for change in changes {
        for npc in &npcs {
            let receiver = npc.instance_id();
            if receiver == change.source {
                continue;
            }
            script.run_passive_perception(
                receiver,
                change.source,
                change.other,
                change.victim,
                change.perception,
            );
        }
    }
}

fn run_npc_perceptions(
    mut updates: MessageReader<PeriodicUpdate<NpcPerceptionKind>>,
    gothic_script: Res<ScriptResource>,
    auto_talk: Res<NpcAutoTalkOptions>,
    dialog_ui: Res<DialogUiState>,
    inventory_ui: Res<InventoryUiState>,
    journal_ui: Res<JournalUiState>,
    spatial_query: SpatialQuery,
    npcs: Query<&GameNpc, (With<AnimationController>, Without<ChildOf>)>,
) {
    let perf = perf_debug!();
    if updates.is_empty() {
        return;
    }
    // Npc initiated dialogs do not interrupt an open dialog or menu
    let auto_talk = auto_talk.enabled
        && !dialog_ui.is_open()
        && !inventory_ui.is_open()
        && !journal_ui.is_open();
    perf.increment_tag("auto_talk", u64::from(auto_talk));
    let mut script = gothic_script.lock();
    let mut effort_us: u64 = 0;
    for update in updates.read() {
        perf.increment_tag("updates", 1);
        let start = Instant::now();
        let Ok(npc) = npcs.get(update.entity) else {
            perf.increment_tag("missing_npc", 1);
            continue;
        };
        let instance = npc.instance_id();
        let interval = script.perception_interval(instance);
        // Applied to the reschedule after the next tick.
        update.control.set_period(Duration::from_secs_f32(
            interval.max(MIN_PERCEPTION_INTERVAL_SECS),
        ));
        perf.increment_tag("scans", 1);
        refresh_visibility(&mut script, &spatial_query, instance);
        script.run_perception(instance, auto_talk);
        let elapsed_us = start.elapsed().as_micros() as u64;
        effort_us += elapsed_us;
        update.control.update_effort(elapsed_us);
    }
    if effort_us > 0 {
        perf.record_count("effort_us", effort_us as usize);
    }
}

fn refresh_visibility(script: &mut ScriptManager, spatial_query: &SpatialQuery, npc: InstanceId) {
    script.refresh_visibility(npc, &mut |observer, target| {
        sight_blocked(spatial_query, observer, target)
    });
}

/// Whether static world geometry separates the two npc roots. Any clear line
/// from the observer eye to the target center or eye counts as visible.
fn sight_blocked(spatial_query: &SpatialQuery, observer: Vec3, target: Vec3) -> bool {
    let from = observer + Vec3::Y * NPC_EYE_HEIGHT;
    [NPC_BODY_CENTER_HEIGHT, NPC_EYE_HEIGHT]
        .into_iter()
        .all(|height| point_blocked(spatial_query, from, target + Vec3::Y * height))
}

fn point_blocked(spatial_query: &SpatialQuery, from: Vec3, to: Vec3) -> bool {
    let Some(direction) = Dir3::new(to - from).ok() else {
        return false;
    };
    spatial_query
        .cast_ray(
            from,
            direction,
            from.distance(to),
            true,
            &SpatialQueryFilter::from_mask(STATIC_OBJECT),
        )
        .is_some()
}
