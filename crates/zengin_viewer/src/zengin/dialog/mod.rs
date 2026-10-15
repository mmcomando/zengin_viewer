mod state;
mod ui;

use crate::audio::sound_manager::PlaySound;
use crate::gameplay::camera::{CameraResource, CurrentCamera};
use crate::gameplay::character::PlayerPhysicsBodyMarker;
use crate::gameplay::objects::PlayerCharacter;
use crate::zengin::GameNpc;
use crate::zengin::selection::{Selection, SelectionKind};
use bevy::log::info;
use bevy::prelude::*;
use state::DialogUiState;

pub(crate) use state::{DialogLineView, DialogOptionView, DialogReply, DialogRequest};

pub struct GameDialogPlugin;

impl Plugin for GameDialogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DialogUiState>();
        app.add_message::<DialogRequest>();
        app.add_message::<DialogReply>();
        app.add_systems(Startup, ui::override_default_font);
        app.add_systems(Startup, ui::spawn_dialog_ui);
        app.add_systems(
            Update,
            (
                show_closest_npc_dialog,
                state::handle_dialog_input,
                apply_dialog_reply,
                state::advance_dialog_playback,
                ui::sync_dialog_ui,
            )
                .chain(),
        );
    }
}

/// Opens the dialog of the currently selected npc on demand, the root options
/// arrive with the script reply
fn show_closest_npc_dialog(
    keys: Res<ButtonInput<KeyCode>>,
    selection: Res<Selection>,
    mut dialog_ui: ResMut<DialogUiState>,
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

    let npc_from = player_npc.npc_instance_id;
    let npc_talk_to = closest_npc.npc_instance_id;
    // The selection system already resolved the npc name
    let generation = dialog_ui.open(
        closest_entity,
        npc_from,
        npc_talk_to,
        selection.name.clone(),
    );
    requests.write(DialogRequest::Open {
        generation,
        npc_from,
        npc_talk_to,
    });
}

/// Applies the script replies to the dialog state. Stale replies (different
/// generation) are dropped.
fn apply_dialog_reply(
    mut replies: MessageReader<DialogReply>,
    mut dialog_ui: ResMut<DialogUiState>,
    mut sounds: MessageWriter<PlaySound>,
    mut requests: MessageWriter<DialogRequest>,
    mut camera_info: ResMut<CameraResource>,
    mut npcs: Query<&mut Transform, Without<PlayerPhysicsBodyMarker>>,
    mut player: Query<&mut Transform, With<PlayerPhysicsBodyMarker>>,
) {
    for reply in replies.read() {
        match reply {
            DialogReply::Options {
                generation,
                options,
            } => {
                if *generation != dialog_ui.generation {
                    continue;
                }
                let awaiting_open = dialog_ui.awaiting_open;
                let Some(options) = options else {
                    dialog_ui.close();
                    continue;
                };
                dialog_ui.accept_options(options.clone());
                if !awaiting_open {
                    continue;
                }
                // The dialog really opened, turn both toward each other and
                // switch to the dialog camera
                let Some(entity) = dialog_ui.npc_entity else {
                    continue;
                };
                let Ok(mut npc_tr) = npcs.get_mut(entity) else {
                    continue;
                };
                let Ok(mut player_tr) = player.single_mut() else {
                    continue;
                };
                face_towards(&mut npc_tr, player_tr.translation);
                face_towards(&mut player_tr, npc_tr.translation);
                camera_info.set_dialog_camera(*npc_tr);
            }
            DialogReply::Lines {
                generation,
                lines,
                sub_choices,
            } => {
                if *generation != dialog_ui.generation {
                    continue;
                }
                dialog_ui.accept_sub_choices(sub_choices.clone());
                if lines.is_empty() {
                    // Nothing was produced (e.g. skipped sequence function), keep
                    // options shown but re-request them as they may have changed
                    dialog_ui.refresh_root_options(&mut requests);
                } else {
                    dialog_ui.begin_line_playback(lines.clone(), &mut sounds);
                }
            }
        }
    }
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
