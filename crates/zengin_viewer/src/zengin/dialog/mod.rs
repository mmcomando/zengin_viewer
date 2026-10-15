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
use crate::zengin::script::state::{DialogLine, DialogRoot};
use crate::zengin::script::symbol_id::InstanceId;
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
        // The UI systems emit requests, `handle_dialog_requests` applies them to
        // the state and `sync_dialog_ui` renders the result, all in one frame.
        app.add_systems(
            Update,
            (
                show_closest_npc_dialog,
                state::handle_dialog_input,
                state::advance_dialog_playback,
                handle_dialog_requests,
                ui::sync_dialog_ui,
            )
                .chain(),
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

/// Applies the dialog requests: runs the dialog script functions and updates
/// [`DialogUiState`], the camera and the speech sounds.
#[allow(clippy::too_many_arguments)]
fn handle_dialog_requests(
    mut requests: MessageReader<DialogRequest>,
    mut dialog_ui: ResMut<DialogUiState>,
    mut sounds: MessageWriter<PlaySound>,
    mut camera_info: ResMut<CameraResource>,
    dialog_texts: Res<DialogTexts>,
    speech_sounds: Res<SpeechSounds>,
    gothic_script: Res<ScriptResource>,
    mut npcs: Query<&mut Transform, Without<PlayerPhysicsBodyMarker>>,
    mut player: Query<&mut Transform, With<PlayerPhysicsBodyMarker>>,
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
                dialog_ui.open(
                    *npc_entity,
                    *npc_from,
                    *npc_talk_to,
                    npc_name.clone(),
                    roots.iter().map(option_view).collect(),
                );
                // Turn both toward each other and switch to the dialog camera
                if let Ok(mut npc_tr) = npcs.get_mut(*npc_entity)
                    && let Ok(mut player_tr) = player.single_mut()
                {
                    face_towards(&mut npc_tr, player_tr.translation);
                    face_towards(&mut player_tr, npc_tr.translation);
                    camera_info.set_dialog_camera(*npc_tr);
                }
            }
            DialogRequest::RefreshRoots {
                npc_from,
                npc_talk_to,
            } => {
                let options = {
                    let mut script = gothic_script.lock();
                    if script.close_dialog() {
                        None
                    } else {
                        npc_dialog_roots(&mut script, *npc_from, *npc_talk_to)
                            .map(|roots| roots.iter().map(option_view).collect())
                    }
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
                let (raw_lines, sub_choices, roots) = {
                    let mut script = gothic_script.lock();
                    script.mark_info_known(*npc_from, *info_instance_id);
                    // Sub choices are cleared before running a root level option
                    script.clear_sub_dialog_choices();
                    script.clear_dialog_lines();
                    // Dialog functions are executed with `self` bound to the npc the
                    // dialog is open with, like the original engine does
                    script.set_self(*npc_talk_to);
                    script.set_other(*npc_from);
                    script.set_instance(*npc_talk_to);
                    script.call_dialog_sequence_function(*sequence_function_id);

                    let sub_choices: Vec<DialogOptionView> = script
                        .sub_dialog_choices()
                        .iter()
                        .map(option_view)
                        .collect();
                    let raw_lines: Vec<DialogLine> = script.dialog_lines().to_vec();
                    // With no lines and no sub choices the root options are
                    // refreshed, the executed option may have changed them
                    let roots = if raw_lines.is_empty() && sub_choices.is_empty() {
                        if script.close_dialog() {
                            Some(None)
                        } else {
                            Some(
                                npc_dialog_roots(&mut script, *npc_from, *npc_talk_to)
                                    .map(|roots| roots.iter().map(option_view).collect()),
                            )
                        }
                    } else {
                        None
                    };
                    (raw_lines, sub_choices, roots)
                };

                let lines = build_line_views(&raw_lines, &dialog_texts, &speech_sounds);
                dialog_ui.accept_sub_choices(sub_choices);
                if !lines.is_empty() {
                    dialog_ui.begin_line_playback(lines, &mut sounds);
                    continue;
                }
                match roots {
                    Some(Some(options)) => dialog_ui.accept_options(options),
                    // The script asked to close the dialog
                    Some(None) => dialog_ui.close(),
                    // Sub choices are shown, nothing else to do
                    None => {}
                }
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
            let sound_path = get_dialog_sound_path(&line.line_str);
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
