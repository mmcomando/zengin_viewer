mod state;
mod ui;

use crate::audio::sound_manager::PlaySound;
use crate::gameplay::camera::{CameraResource, CurrentCamera};
use crate::gameplay::character::PlayerPhysicsBodyMarker;
use crate::gameplay::objects::PlayerCharacter;
use crate::zengin::GameNpc;
use crate::zengin::common::get_dialog_sound_path;
use crate::zengin::script::component::{DialogTexts, ScriptResource};
use crate::zengin::script::manager::ScriptManager;
use crate::zengin::script::state::{DialogClose, DialogLine, DialogRoot};
use crate::zengin::script::symbol_id::InstanceId;
use crate::zengin::script_bridge::ScriptChangesApplied;
use crate::zengin::selection::{Selection, SelectionKind};
use crate::zengin::sounds::SpeechSounds;
use bevy::log::{info, warn};
use bevy::prelude::*;
use state::{DialogLineView, DialogOptionView, DialogRequest, DialogUiState};

pub struct GameDialogPlugin;

impl Plugin for GameDialogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DialogUiState>();
        app.add_message::<DialogRequest>();
        app.add_systems(Startup, ui::override_default_font);
        app.add_systems(Startup, ui::spawn_dialog_ui);
        // The UI systems emit requests, `handle_dialog_requests` applies them
        // and runs the script before the script bridge drains the produced
        // outputs, `handle_dialog_outputs` consumes them in the same frame,
        // `aim_dialog_camera` points the camera at the current speaker and
        // `sync_dialog_ui` renders the result.
        app.add_systems(
            Update,
            (
                show_closest_npc_dialog,
                state::handle_dialog_input,
                state::advance_dialog_playback,
                handle_dialog_requests,
            )
                .chain()
                .before(ScriptChangesApplied),
        );
        app.add_systems(
            Update,
            (handle_dialog_outputs, aim_dialog_camera, ui::sync_dialog_ui)
                .chain()
                .after(ScriptChangesApplied),
        );
    }
}

/// Opens the dialog of the currently selected npc on demand
fn show_closest_npc_dialog(
    keys: Res<ButtonInput<KeyCode>>,
    selection: Res<Selection>,
    dialog_ui: Res<DialogUiState>,
    mut camera_info: ResMut<CameraResource>,
    mut requests: MessageWriter<DialogRequest>,
    npcs: Query<&GameNpc>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
) {
    if !dialog_ui.is_open() && camera_info.current_camera == CurrentCamera::Dialog {
        camera_info.current_camera = CurrentCamera::ThirdPerson;
        return;
    }

    // TODO action instead of key
    if !keys.just_pressed(KeyCode::KeyE) {
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
fn handle_dialog_outputs(
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
fn handle_dialog_requests(
    mut requests: MessageReader<DialogRequest>,
    mut dialog_ui: ResMut<DialogUiState>,
    gothic_script: Res<ScriptResource>,
    mut npcs: Query<&mut Transform, Without<PlayerPhysicsBodyMarker>>,
    mut player: Query<(Entity, &mut Transform), With<PlayerPhysicsBodyMarker>>,
    player_visual: Query<Entity, With<PlayerCharacter>>,
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
                let Ok((_, mut player_tr)) = player.single_mut() else {
                    continue;
                };
                let Ok(player_visual) = player_visual.single() else {
                    continue;
                };
                dialog_ui.open(
                    *npc_entity,
                    player_visual,
                    *npc_from,
                    *npc_talk_to,
                    npc_name.clone(),
                    roots.iter().map(option_view).collect(),
                );
                // Turn both toward each other, the camera is aimed afterwards by
                // `aim_dialog_camera`
                if let Ok(mut npc_tr) = npcs.get_mut(*npc_entity) {
                    face_towards(&mut npc_tr, player_tr.translation);
                    face_towards(&mut player_tr, npc_tr.translation);
                }
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

/// How far the camera is offset to the side from directly behind the listening
/// character, as a fraction of the character separation. The side is fixed to
/// the pair of characters so the shot does not mirror when the speaker changes.
const DIALOG_CAMERA_SIDE: f32 = 0.6;

/// Places the dialog camera behind the character that is currently listening,
/// looking back at the speaker, with its distance scaled to how far the two
/// talking characters stand apart.
fn aim_dialog_camera(
    dialog_ui: Res<DialogUiState>,
    mut camera_info: ResMut<CameraResource>,
    transforms: Query<&GlobalTransform>,
) {
    if !dialog_ui.is_open() {
        return;
    }
    let (Some((viewed, other)), Some(player), Some(npc)) = (
        dialog_ui.camera_characters(),
        dialog_ui.player_entity,
        dialog_ui.npc_entity,
    ) else {
        return;
    };
    let (Ok(viewed_tr), Ok(other_tr), Ok(player_tr), Ok(npc_tr)) = (
        transforms.get(viewed),
        transforms.get(other),
        transforms.get(player),
        transforms.get(npc),
    ) else {
        return;
    };

    let viewed_pos = viewed_tr.translation();
    let other_pos = other_tr.translation();
    let separation = viewed_pos.distance(other_pos);
    // Behind the listening character is away from the speaking one, offset to a
    // fixed side of the pair so the shot does not mirror when the speaker changes
    let behind = (other_pos - viewed_pos).normalize_or_zero();
    let pair = (player_tr.translation() - npc_tr.translation()).normalize_or_zero();
    let side = Vec3::Y.cross(pair).normalize_or_zero();
    let behind = (behind + side * DIALOG_CAMERA_SIDE).normalize_or_zero();
    // The camera sits behind the listener and looks back at the speaker (`viewed`)
    let camera_pos = other_pos + behind * separation;
    let to_camera = Vec3::new(
        camera_pos.x - viewed_pos.x,
        0.0,
        camera_pos.z - viewed_pos.z,
    );
    let distance = to_camera.length();

    let reference = Transform::from_translation(viewed_pos)
        .with_rotation(Quat::from_rotation_y(to_camera.x.atan2(to_camera.z)));
    camera_info.set_dialog_camera(reference, distance);
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
fn face_towards(tr: &mut Transform, target: Vec3) {
    let delta = target - tr.translation;
    let dir = Vec3::new(delta.x, 0.0, delta.z);
    if dir.length_squared() < f32::EPSILON {
        return;
    }
    let dir = dir.normalize();
    tr.rotation = Quat::from_rotation_y(dir.x.atan2(dir.z));
}
