//! Dialog flow systems: opening a dialog from the selection, running the
//! script for requested options and applying the produced outputs to the
//! dialog state.

use super::sounds::SpeechSounds;
use super::state::{DialogLineView, DialogOptionView, DialogRequest, DialogUiState};
use crate::audio::PlaySound;
use crate::gameplay::camera::{CameraResource, CurrentCamera};
use crate::gameplay::objects::PlayerCharacter;
use crate::gui::OpenTrade;
use crate::input::{GameplayAction, GameplayInputMarker};
use crate::zengin::GameNpc;
use crate::zengin::common::get_dialog_sound_path;
use crate::zengin::instance_index::InstanceIndex;
use crate::zengin::interaction::selection::{Selection, SelectionKind};
use crate::zengin::npc_routines::NpcWeaponMode;
use crate::zengin::script::component::{DialogTexts, ScriptResource};
use crate::zengin::script::manager::ScriptManager;
use crate::zengin::script::state::{
    DialogChoice, DialogClose, DialogLine, DialogRoot, ProcessInfosChange, WeaponMode,
};
use crate::zengin::script::symbol_id::{FunctionId, InstanceId};
use bevy::log::{info, warn};
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;
use std::collections::HashSet;

/// Opens the dialog of the currently selected npc on demand
#[allow(clippy::too_many_arguments)]
pub(super) fn show_closest_npc_dialog(
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    selection: Res<Selection>,
    dialog_ui: Res<DialogUiState>,
    mut camera_info: ResMut<CameraResource>,
    mut requests: MessageWriter<DialogRequest>,
    npcs: Query<&GameNpc>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
    player_weapon: Query<&NpcWeaponMode, With<PlayerCharacter>>,
) {
    if !dialog_ui.is_open() && camera_info.current() == CurrentCamera::Dialog {
        camera_info.set_third_person_camera();
        return;
    }

    if !action_state.just_pressed(&GameplayAction::Interact) {
        return;
    }

    // With a weapon drawn Interact attacks instead of talking.
    let weapon_mode = player_weapon
        .single()
        .map_or(WeaponMode::Fists, |mode| mode.0);
    if weapon_mode != WeaponMode::Fists {
        return;
    }

    // Dialog already open, it stays open until EXIT is selected
    if dialog_ui.is_open() {
        return;
    }

    // Only npcs are talkable
    if selection.kind != Some(SelectionKind::Npc) {
        return;
    }
    let Some(closest_entity) = selection.target else {
        return;
    };
    let Ok(closest_npc) = npcs.get(closest_entity) else {
        return;
    };
    let Ok(player_npc) = player_npc.single() else {
        return;
    };

    info!("Selected npc: {}", closest_npc.instance_id());

    requests.write(DialogRequest::Talk {
        npc_entity: closest_entity,
        npc_from: player_npc.instance_id(),
        npc_talk_to: closest_npc.instance_id(),
        // The selection system already resolved the npc name
        npc_name: selection.name.clone(),
    });
}

/// Opens the dialog requested by `AI_ProcessInfos`. The script state
/// (`zs_talk`) decides when a dialog starts, this system only resolves the
/// entities and forwards the request to the dialog handler.
pub(super) fn show_requested_npc_dialog(
    mut changes: MessageReader<ProcessInfosChange>,
    index: Res<InstanceIndex>,
    dialog_ui: Res<DialogUiState>,
    gothic_script: Res<ScriptResource>,
    mut requests: MessageWriter<DialogRequest>,
) {
    for change in changes.read() {
        if dialog_ui.is_open() {
            continue;
        }
        let Some(npc_entity) = index.npcs(change.npc).first().copied() else {
            warn!("AI_ProcessInfos npc({}) has no entity", change.npc);
            continue;
        };
        let npc_name = {
            let mut script = gothic_script.lock();
            script
                .npc_display_name(change.npc)
                .unwrap_or_else(|| script.call_get_name(change.npc.symbol()))
        };
        requests.write(DialogRequest::Open {
            npc_entity,
            npc_from: change.other,
            npc_talk_to: change.npc,
            npc_name,
        });
        // One requested dialog per frame
        return;
    }
}

/// Ends the dialog when the script asked for it via `AI_StopProcessInfos`, and
pub(super) fn handle_dialog_outputs(
    mut lines: MessageReader<DialogLine>,
    mut closes: MessageReader<DialogClose>,
    mut dialog_ui: ResMut<DialogUiState>,
    mut sounds: MessageWriter<PlaySound>,
    mut trade_requests: MessageWriter<OpenTrade>,
    dialog_texts: Res<DialogTexts>,
    speech_sounds: Res<SpeechSounds>,
    gothic_script: Res<ScriptResource>,
) {
    // Messages are always read, stale outputs produced outside an executed
    // option are dropped instead of leaking into the next capture
    let new_lines: Vec<DialogLine> = lines.read().cloned().collect();
    let mut close_requested = false;
    for _ in closes.read() {
        close_requested = true;
    }
    let awaiting = dialog_ui.take_awaiting_outputs();
    let executed_info = dialog_ui.take_pending_info();

    // A close request outside an executed option applies directly
    if close_requested && !awaiting {
        dialog_ui.request_close();
        return;
    }
    if !awaiting {
        return;
    }

    let line_views = build_line_views(&new_lines, &dialog_texts, &speech_sounds);
    // The choices after an option are its info's sub choices, owned by the
    // script. An option that adds none leaves the previous ones shown instead of
    // falling back to the root options.
    let choice_views: Vec<DialogOptionView> = match executed_info {
        Some(info) => {
            let mut script = gothic_script.lock();
            script
                .sub_dialog_choices(info)
                .iter()
                .map(|choice| choice_view(choice, info))
                .collect()
        }
        None => Vec::new(),
    };

    let has_sub_choices = !choice_views.is_empty();
    dialog_ui.accept_sub_choices(choice_views);
    if !line_views.is_empty() {
        dialog_ui.begin_line_playback(line_views, &mut sounds);
        if close_requested {
            // The close is deferred until the playback finished
            dialog_ui.request_close();
        }
        return;
    }
    if close_requested {
        dialog_ui.close();
        return;
    }
    if dialog_ui.take_pending_trade() {
        let (Some(player), Some(trader)) = (dialog_ui.npc_player, dialog_ui.npc_talk_to) else {
            return;
        };
        trade_requests.write(OpenTrade { player, trader });
        return;
    }
    if has_sub_choices {
        // Only sub choices were produced, they are shown already
        return;
    }
    // With no lines and no sub choices the root options are refreshed, the
    // executed option may have changed them
    let options = {
        let (Some(npc_from), Some(npc_talk_to)) = (dialog_ui.npc_player, dialog_ui.npc_talk_to)
        else {
            return;
        };
        let except = dialog_ui.executed_functions().clone();
        let mut script = gothic_script.lock();
        npc_dialog_roots(&mut script, npc_from, npc_talk_to, &except, false)
            .map(|roots| roots.iter().map(option_view).collect::<Vec<_>>())
    };
    match options {
        // No options left, the dialog has nothing more to offer
        Some(options) if options.is_empty() => dialog_ui.close(),
        Some(options) => dialog_ui.accept_options(options),
        None => dialog_ui.close(),
    }
}

/// Applies the dialog requests: runs the dialog script functions and updates
/// the npc/player facing.
#[allow(clippy::type_complexity, clippy::cognitive_complexity)]
pub(super) fn handle_dialog_requests(
    mut requests: MessageReader<DialogRequest>,
    mut dialog_ui: ResMut<DialogUiState>,
    gothic_script: Res<ScriptResource>,
    mut npcs: Query<(&mut Transform, &GlobalTransform), Without<PlayerCharacter>>,
    mut player: Query<(Entity, &mut Transform, &GlobalTransform), With<PlayerCharacter>>,
) {
    for request in requests.read() {
        match request {
            DialogRequest::Talk {
                npc_entity,
                npc_from,
                npc_talk_to,
                npc_name,
            } => {
                // A dialog opened earlier in this frame wins
                if dialog_ui.is_open() {
                    continue;
                }
                let started = {
                    let mut script = gothic_script.lock();
                    script.call_assesstalk(*npc_from, *npc_talk_to)
                };
                if started {
                    continue;
                }
                warn!(
                    "b_assesstalk is missing, opening the dialog with npc({npc_talk_to}) directly"
                );
                open_dialog(
                    &mut dialog_ui,
                    &gothic_script,
                    &mut npcs,
                    &mut player,
                    *npc_entity,
                    *npc_from,
                    *npc_talk_to,
                    npc_name.clone(),
                );
            }
            DialogRequest::Open {
                npc_entity,
                npc_from,
                npc_talk_to,
                npc_name,
            } => {
                if dialog_ui.is_open() {
                    continue;
                }
                open_dialog(
                    &mut dialog_ui,
                    &gothic_script,
                    &mut npcs,
                    &mut player,
                    *npc_entity,
                    *npc_from,
                    *npc_talk_to,
                    npc_name.clone(),
                );
            }
            DialogRequest::RefreshRoots {
                npc_from,
                npc_talk_to,
            } => {
                let options = {
                    let except = dialog_ui.executed_functions().clone();
                    let mut script = gothic_script.lock();
                    npc_dialog_roots(&mut script, *npc_from, *npc_talk_to, &except, false)
                        .map(|roots| roots.iter().map(option_view).collect::<Vec<_>>())
                };
                match options {
                    Some(options) if options.is_empty() => dialog_ui.close(),
                    Some(options) => dialog_ui.accept_options(options),
                    None => dialog_ui.close(),
                }
            }
            DialogRequest::ChooseOption {
                npc_from,
                npc_talk_to,
                sequence_function_id,
                info_instance_id,
                from_sub_choices,
                trade,
            } => {
                if dialog_ui.npc_talk_to != Some(*npc_talk_to) {
                    warn!(
                        "Dialog option for npc({npc_talk_to}) ignored, the open dialog belongs to {:?}",
                        dialog_ui.npc_talk_to
                    );
                    continue;
                }
                let mut script = gothic_script.lock();
                execute_dialog_function(
                    &mut script,
                    &mut dialog_ui,
                    *npc_from,
                    *npc_talk_to,
                    *sequence_function_id,
                    *info_instance_id,
                    *from_sub_choices,
                    *trade,
                );
            }
        }
    }
}

/// Resolves the npc's roots and opens the dialog UI. The first undescribed
/// important info is executed immediately, matching the engine's dialog start.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn open_dialog(
    dialog_ui: &mut DialogUiState,
    gothic_script: &ScriptResource,
    npcs: &mut Query<(&mut Transform, &GlobalTransform), Without<PlayerCharacter>>,
    player: &mut Query<(Entity, &mut Transform, &GlobalTransform), With<PlayerCharacter>>,
    npc_entity: Entity,
    npc_from: InstanceId,
    npc_talk_to: InstanceId,
    npc_name: String,
) -> bool {
    let roots = {
        let mut script = gothic_script.lock();
        npc_dialog_roots(&mut script, npc_from, npc_talk_to, &HashSet::new(), true)
    };
    let Some(roots) = roots else {
        warn!("No dialog_roots for npc({npc_talk_to}), player({npc_from})");
        dialog_ui.close();
        return false;
    };
    bevy::log::debug!(
        "Dialog open npc({}) player({}) roots({})",
        npc_talk_to.value(),
        npc_from.value(),
        roots.len()
    );
    let Ok((player_entity, mut player_tr, player_global)) = player.single_mut() else {
        return false;
    };
    dialog_ui.open(
        npc_entity,
        player_entity,
        npc_from,
        npc_talk_to,
        npc_name,
        roots.iter().map(option_view).collect(),
    );
    face_dialog_characters(npc_entity, &mut player_tr, player_global, npcs);

    let auto = roots
        .first()
        .filter(|root| root.important && root.description.is_empty())
        .cloned();
    let Some(root) = auto else {
        return true;
    };
    let mut script = gothic_script.lock();
    execute_dialog_function(
        &mut script,
        dialog_ui,
        npc_from,
        npc_talk_to,
        root.sequence_function_id,
        root.info_instance_id,
        false,
        root.trade,
    );
    true
}

#[allow(clippy::type_complexity)]
fn face_dialog_characters(
    npc_entity: Entity,
    player_tr: &mut Transform,
    player_global: &GlobalTransform,
    npcs: &mut Query<(&mut Transform, &GlobalTransform), Without<PlayerCharacter>>,
) {
    let Ok((mut npc_tr, npc_global)) = npcs.get_mut(npc_entity) else {
        warn!("Dialog npc({npc_entity}) not found, cannot face dialog characters");
        return;
    };
    face_towards(
        &mut npc_tr,
        npc_global.translation(),
        player_global.translation(),
    );
    face_towards(
        player_tr,
        player_global.translation(),
        npc_global.translation(),
    );
}

/// Fetches the npc's condition-valid root options. When important roots are
/// requested they replace the normal ones, matching the original choice pass;
/// permanent important roots already executed in this session are skipped.
fn npc_dialog_roots(
    gothic_script: &mut ScriptManager,
    npc_player: InstanceId,
    npc_talk_to: InstanceId,
    except: &HashSet<FunctionId>,
    include_important: bool,
) -> Option<Vec<DialogRoot>> {
    let roots = gothic_script.refresh_npc_check_info(npc_talk_to, npc_player)?;
    if include_important {
        let important: Vec<DialogRoot> = roots
            .iter()
            .filter(|root| root.important)
            .filter(|root| !(root.permanent && except.contains(&root.sequence_function_id)))
            .cloned()
            .collect();
        if !important.is_empty() {
            return Some(important);
        }
    }
    Some(roots.into_iter().filter(|root| !root.important).collect())
}

/// Runs a dialog choice's information function and starts capturing its outputs.
/// A root choice marks its info known, a sub choice is a plain function of an
/// already known info.
#[allow(clippy::too_many_arguments)]
fn execute_dialog_function(
    gothic_script: &mut ScriptManager,
    dialog_ui: &mut DialogUiState,
    npc_from: InstanceId,
    npc_talk_to: InstanceId,
    sequence_function_id: FunctionId,
    info_instance_id: Option<InstanceId>,
    from_sub_choices: bool,
    trade: bool,
) {
    if from_sub_choices {
        if let Some(info) = info_instance_id {
            gothic_script.remove_sub_dialog_choice(info, sequence_function_id);
        }
    } else {
        gothic_script.mark_info_known(npc_from, info_instance_id);
    }
    dialog_ui.record_executed(sequence_function_id);
    gothic_script.clear_dialog_lines();
    // Dialog functions are executed with `self` bound to the npc the dialog is
    // open with, like the original engine does
    gothic_script.set_self(npc_talk_to);
    gothic_script.set_other(npc_from);
    gothic_script.set_instance(npc_talk_to);
    gothic_script.call_dialog_sequence_function(sequence_function_id);
    dialog_ui.begin_output_capture(info_instance_id, trade);
}

/// Strips a script dialog root down to the data the UI needs
fn option_view(root: &DialogRoot) -> DialogOptionView {
    DialogOptionView {
        description: root.description.clone(),
        sequence_function_id: root.sequence_function_id,
        info_instance_id: root.info_instance_id,
        trade: root.trade,
    }
}

/// Strips a script sub choice down to the data the UI needs, tagged with the
/// owning info.
fn choice_view(choice: &DialogChoice, info_instance_id: InstanceId) -> DialogOptionView {
    DialogOptionView {
        description: choice.description.clone(),
        sequence_function_id: choice.sequence_function_id,
        info_instance_id: Some(info_instance_id),
        trade: false,
    }
}

/// Resolves display text and sound for the script's dialog lines
fn build_line_views(
    lines: &[DialogLine],
    dialog_texts: &DialogTexts,
    speech_sounds: &SpeechSounds,
) -> Vec<DialogLineView> {
    lines
        .iter()
        .map(|line| {
            let text = dialog_texts.get(&line.line_str).cloned();
            if text.is_none() {
                warn!(
                    "Missing dialog text: npc_a({}) npc_b({}) line_id({}) block({})",
                    line.npc_a, line.npc_b, line.line_id, line.line_str
                );
            }
            let sound_path = get_dialog_sound_path(line.line_str.as_str());
            let sound_duration = speech_sounds.duration(&sound_path);
            DialogLineView {
                text,
                sound_path,
                sound_duration,
                npc_a: line.npc_a,
                npc_b: line.npc_b,
                line_id: line.line_id,
            }
        })
        .collect()
}

/// Horizontally rotates the transform so its facing axis (+Z) points at `target`
/// Positions are in the transform's own coordinate space, i.e. its translation
/// for a root entity or the global positions for world-space facing.
fn face_towards(tr: &mut Transform, position: Vec3, target: Vec3) {
    let Some(rotation) = horizontal_facing_rotation(position, target) else {
        return;
    };
    tr.rotation = rotation;
}

/// Yaw rotation (+Z facing) pointing from `position` horizontally at `target`,
/// `None` when the two positions coincide horizontally.
fn horizontal_facing_rotation(position: Vec3, target: Vec3) -> Option<Quat> {
    let delta = target - position;
    let dir = Vec3::new(delta.x, 0.0, delta.z);
    if dir.length_squared() < f32::EPSILON {
        return None;
    }
    let dir = dir.normalize();
    Some(Quat::from_rotation_y(dir.x.atan2(dir.z)))
}
