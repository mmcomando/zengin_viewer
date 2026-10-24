//! Npc initiated dialogs: npcs start their important infos automatically when
//! the player comes close, without the player having to interact with them.

use std::collections::{HashMap, HashSet};

use bevy::log::info;
use bevy::prelude::*;

use crate::gameplay::character::PlayerPhysicsBodyMarker;
use crate::gameplay::objects::PlayerCharacter;
use crate::gui::InventoryUiState;
use crate::zengin::GameNpc;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::symbol_id::InstanceId;

use super::state::{DialogRequest, DialogUiState};

/// Distance (meters) within which an npc may start an important dialog.
const IMPORTANT_DIALOG_RANGE: f32 = 5.0;

/// How often the important info conditions of a single npc are evaluated while
/// the player stays near it.
const IMPORTANT_DIALOG_RECHECK_SECONDS: f64 = 2.0;

pub(super) struct ImportantDialogsPlugin;

impl Plugin for ImportantDialogsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ImportantDialogChecks>();
    }
}

/// Condition evaluation throttle of the npc initiated dialogs.
#[derive(Resource, Default)]
pub(super) struct ImportantDialogChecks {
    /// Earliest time the important conditions of an npc may be evaluated again.
    /// Entries are dropped when the npc leaves the trigger range.
    next_check: HashMap<InstanceId, f64>,
}

/// Starts an important dialog of the nearest npc whose condition allows it.
/// The cheap checks (has pending infos, recheck throttle) run every frame,
/// condition functions at most once per [`IMPORTANT_DIALOG_RECHECK_SECONDS`].
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn auto_start_important_dialogs(
    time: Res<Time>,
    gothic_script: Res<ScriptResource>,
    dialog_ui: Res<DialogUiState>,
    inventory_ui: Res<InventoryUiState>,
    mut checks: ResMut<ImportantDialogChecks>,
    player: Query<&GlobalTransform, With<PlayerPhysicsBodyMarker>>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
    npcs: Query<
        (Entity, &GameNpc, &GlobalTransform),
        (Without<PlayerCharacter>, Without<PlayerPhysicsBodyMarker>),
    >,
    mut requests: MessageWriter<DialogRequest>,
) {
    if dialog_ui.is_open() || inventory_ui.is_open() {
        return;
    }
    let Ok(player_tr) = player.single() else {
        return;
    };
    let Ok(player_npc) = player_npc.single() else {
        return;
    };
    let player_id = player_npc.instance_id();
    let player_pos = player_tr.translation();

    let mut near: Vec<(f32, Entity, InstanceId)> = npcs
        .iter()
        .filter_map(|(entity, npc, tr)| {
            let distance_sq = tr.translation().distance_squared(player_pos);
            (distance_sq <= IMPORTANT_DIALOG_RANGE * IMPORTANT_DIALOG_RANGE).then_some((
                distance_sq,
                entity,
                npc.instance_id(),
            ))
        })
        .collect();
    near.sort_by(|left, right| left.0.total_cmp(&right.0));

    let near_ids: HashSet<InstanceId> = near.iter().map(|(_, _, id)| *id).collect();
    checks.next_check.retain(|id, _| near_ids.contains(id));
    if near.is_empty() {
        return;
    }

    let now = time.elapsed_secs_f64();
    let mut script = gothic_script.lock();
    for (_, entity, npc_id) in near {
        if !script.npc_has_pending_important_dialog(player_id, npc_id) {
            continue;
        }
        if script
            .npc_instances()
            .get(&npc_id)
            .is_some_and(|npc| npc.hitpoints.is_some_and(|hitpoints| hitpoints <= 0))
        {
            continue;
        }
        let next_check = checks
            .next_check
            .get(&npc_id)
            .copied()
            .unwrap_or(f64::NEG_INFINITY);
        if now < next_check {
            continue;
        }
        // Conditions run at most once per recheck interval, the timestamp is
        // updated even when no condition matches
        checks
            .next_check
            .insert(npc_id, now + IMPORTANT_DIALOG_RECHECK_SECONDS);

        let Some(root) = script.npc_ready_important_dialog(player_id, npc_id) else {
            continue;
        };
        drop(script);

        info!(
            "Npc({npc_id}) starts important dialog info({:?})",
            root.info_instance_id
        );
        requests.write(DialogRequest::OpenImportant {
            npc_entity: entity,
            npc_from: player_id,
            npc_talk_to: npc_id,
            sequence_function_id: root.sequence_function_id,
            info_instance_id: root.info_instance_id,
        });
        // One initiated dialog per frame
        return;
    }
}
