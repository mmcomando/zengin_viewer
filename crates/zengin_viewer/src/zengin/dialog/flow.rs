//! Dialog flow systems: opening a dialog from the selection, running the
//! script for requested options and applying the produced outputs to the
//! dialog state.

use super::sounds::SpeechSounds;
use super::state::{DialogLineView, DialogOptionView, DialogRequest, DialogUiState};
use crate::audio::PlaySound;
use crate::gameplay::camera::{CameraResource, CurrentCamera};
use crate::gameplay::character::PlayerPhysicsBodyMarker;
use crate::gameplay::objects::PlayerCharacter;
use crate::input::{GameplayAction, GameplayInputMarker};
use crate::zengin::GameNpc;
use crate::zengin::common::get_dialog_sound_path;
use crate::zengin::interaction::selection::{Selection, SelectionKind};
use crate::zengin::npc_routines::NpcWeaponMode;
use crate::zengin::script::component::{DialogTexts, ScriptResource};
use crate::zengin::script::manager::ScriptManager;
use crate::zengin::script::state::{DialogClose, DialogLine, DialogRoot, WeaponMode};
use crate::zengin::script::symbol_id::InstanceId;
use bevy::log::{info, warn};
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

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
    if !dialog_ui.is_open() && camera_info.current_camera == CurrentCamera::Dialog {
        camera_info.current_camera = CurrentCamera::ThirdPerson;
        return;
    }

    if !action_state.just_pressed(&GameplayAction::Interact) {
        return;
    }

    // With a weapon drawn Interact attacks instead of talking.
    let weapon_mode = player_weapon
        .single()
        .map(|mode| mode.0)
        .unwrap_or(WeaponMode::Fists);
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

    info!("Selected npc: {}", closest_npc.npc_instance_id);

    requests.write(DialogRequest::Open {
        npc_entity: closest_entity,
        npc_from: player_npc.npc_instance_id,
        npc_talk_to: closest_npc.npc_instance_id,
        // The selection system already resolved the npc name
        npc_name: selection.name.clone(),
    });
}

/// Ends the dialog when the script asked for it via `AI_StopProcessInfos`, and
/// applies the outputs (`AI_Output` lines, `Info_AddChoice` sub choices) of the
/// option that was just executed. While lines are still playing a close is
/// deferred until the playback finished, like the original engine does.
pub(super) fn handle_dialog_outputs(
    mut lines: MessageReader<DialogLine>,
    mut choices: MessageReader<DialogRoot>,
    mut closes: MessageReader<DialogClose>,
    mut dialog_ui: ResMut<DialogUiState>,
    mut sounds: MessageWriter<PlaySound>,
    dialog_texts: Res<DialogTexts>,
    speech_sounds: Res<SpeechSounds>,
    gothic_script: Res<ScriptResource>,
) {
    // Messages are always read, stale outputs produced outside an executed
    // option are dropped instead of leaking into the next capture
    let new_lines: Vec<DialogLine> = lines.read().cloned().collect();
    let mut new_choices: Vec<DialogRoot> = choices.read().cloned().collect();
    let mut close_requested = false;
    for _ in closes.read() {
        close_requested = true;
    }
    let awaiting = dialog_ui.take_awaiting_outputs();

    // A close request outside an executed option applies directly
    if close_requested && !awaiting {
        dialog_ui.request_close();
        return;
    }
    if !awaiting {
        return;
    }

    let line_views = build_line_views(&new_lines, &dialog_texts, &speech_sounds);
    // Sub choices are shown in reverse addition order, like the previous
    // script state buffer did by inserting at the front
    new_choices.sort_by(|a, b| b.order.cmp(&a.order));
    let choice_views: Vec<DialogOptionView> = new_choices.iter().map(option_view).collect();

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
    if !new_choices.is_empty() {
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
        let mut script = gothic_script.lock();
        npc_dialog_roots(&mut script, npc_from, npc_talk_to)
            .map(|roots| roots.iter().map(option_view).collect())
    };
    match options {
        Some(options) => dialog_ui.accept_options(options),
        None => dialog_ui.close(),
    }
}

/// Applies the dialog requests: runs the dialog script functions and updates
/// the npc/player facing.
pub(super) fn handle_dialog_requests(
    mut requests: MessageReader<DialogRequest>,
    mut dialog_ui: ResMut<DialogUiState>,
    gothic_script: Res<ScriptResource>,
    mut npcs: Query<
        (&mut Transform, &GlobalTransform),
        (Without<PlayerPhysicsBodyMarker>, Without<PlayerCharacter>),
    >,
    mut player_visual: Query<
        (Entity, &mut Transform, &GlobalTransform),
        (With<PlayerCharacter>, Without<PlayerPhysicsBodyMarker>),
    >,
    parents: Query<&ChildOf>,
    global_transforms: Query<&GlobalTransform>,
) {
    for request in requests.read() {
        match request {
            DialogRequest::Open {
                npc_entity,
                npc_from,
                npc_talk_to,
                npc_name,
            } => {
                let roots = {
                    let mut script = gothic_script.lock();
                    npc_dialog_roots(&mut script, *npc_from, *npc_talk_to)
                };
                let Some(roots) = roots else {
                    warn!("No dialog_roots for npc({npc_talk_to}), player({npc_from})");
                    dialog_ui.close();
                    continue;
                };
                let Ok((player_visual_entity, mut player_visual_tr, player_visual_global)) =
                    player_visual.single_mut()
                else {
                    continue;
                };
                dialog_ui.open(
                    *npc_entity,
                    player_visual_entity,
                    *npc_from,
                    *npc_talk_to,
                    npc_name.clone(),
                    roots.iter().map(option_view).collect(),
                );
                // Turn both toward each other, the camera is aimed afterwards by
                // `aim_dialog_camera`. The player visual model is rotated, not
                // the physics body: the sequencer drives the model facing and
                // the root motion direction from the model's global rotation,
                // a stale body yaw would skew post-dialog movement off the
                // camera direction.
                // Facing is computed from global positions and rotations: the
                // visual is a child of the physics body, whose initial rotation
                // must not leak into the model's world facing.
                let Ok((mut npc_tr, npc_global)) = npcs.get_mut(*npc_entity) else {
                    warn!("Dialog npc({npc_entity}) not found, cannot face dialog characters");
                    continue;
                };
                face_towards(
                    &mut npc_tr,
                    npc_global.translation(),
                    player_visual_global.translation(),
                );
                let Ok(parent_global) = parents
                    .get(player_visual_entity)
                    .and_then(|parent| global_transforms.get(parent.0))
                else {
                    warn!(
                        "Player visual({player_visual_entity}) body has no global transform, cannot face dialog npc"
                    );
                    continue;
                };
                face_towards_world(
                    &mut player_visual_tr,
                    parent_global,
                    player_visual_global.translation(),
                    npc_global.translation(),
                );
            }
            DialogRequest::RefreshRoots {
                npc_from,
                npc_talk_to,
            } => {
                let options = {
                    let mut script = gothic_script.lock();
                    npc_dialog_roots(&mut script, *npc_from, *npc_talk_to)
                        .map(|roots| roots.iter().map(option_view).collect())
                };
                match options {
                    None => dialog_ui.close(),
                    Some(options) => dialog_ui.accept_options(options),
                }
            }
            DialogRequest::ChooseOption {
                npc_from,
                npc_talk_to,
                sequence_function_id,
                info_instance_id,
            } => {
                let mut script = gothic_script.lock();
                script.mark_info_known(*npc_from, *info_instance_id);
                // Sub choices and lines are cleared before running a root level
                // option, only outputs of this option are captured
                script.clear_dialog_outputs();
                // Dialog functions are executed with `self` bound to the npc the
                // dialog is open with, like the original engine does
                script.set_self(*npc_talk_to);
                script.set_other(*npc_from);
                script.set_instance(*npc_talk_to);
                script.call_dialog_sequence_function(*sequence_function_id);
                drop(script);
                dialog_ui.begin_output_capture();
            }
        }
    }
}

/// Fetches the npc's root dialog options from the script state, filters them
/// by their condition functions and drops the important ones
fn npc_dialog_roots(
    gothic_script: &mut ScriptManager,
    npc_player: InstanceId,
    npc_talk_to: InstanceId,
) -> Option<Vec<DialogRoot>> {
    let mut roots = gothic_script
        .get_loaded_state()
        .npc_instances
        .get(&npc_talk_to)?
        .dialog_roots
        .clone()?;
    gothic_script.filter_dialog_roots(npc_player, npc_talk_to, &mut roots);
    roots.retain(|root| !root.important);
    Some(roots)
}

/// Strips a script dialog root down to the data the UI needs
fn option_view(root: &DialogRoot) -> DialogOptionView {
    DialogOptionView {
        description: root.description.clone(),
        sequence_function_id: root.sequence_function_id,
        info_instance_id: root.info_instance_id,
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

/// Like [`face_towards`] for a transform nested under a parent: the desired
/// world facing is converted into a local rotation so the parent's own
/// (possibly stale or initial) rotation does not influence the result.
fn face_towards_world(
    tr: &mut Transform,
    parent_global: &GlobalTransform,
    global_position: Vec3,
    target: Vec3,
) {
    let Some(world_rotation) = horizontal_facing_rotation(global_position, target) else {
        return;
    };
    tr.rotation = parent_global.rotation().inverse() * world_rotation;
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
