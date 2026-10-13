mod state;
mod ui;

use crate::gameplay::camera::{CameraResource, CurrentCamera};
use crate::gameplay::character::{PlayerCharacter, PlayerPhysicsBodyMarker};
use crate::zengin::GameNpc;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::main::InstanceId;
use crate::zengin::script::main::ScriptManager;
use crate::zengin::script::state::DialogRoot;
use bevy::prelude::*;
use state::DialogUiState;

pub struct GameDialogPlugin;

impl Plugin for GameDialogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DialogUiState>();
        app.add_systems(Startup, ui::override_default_font);
        app.add_systems(Startup, ui::spawn_dialog_ui);
        app.add_systems(
            Update,
            (
                show_closest_npc_dialog,
                state::handle_dialog_input,
                state::advance_dialog_playback,
                ui::sync_dialog_ui,
            )
                .chain(),
        );
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
        .dialog_roots
        .get(&npc_talk_to)?
        .clone();
    gothic_script.filter_dialog_roots(npc_player, npc_talk_to, &mut roots);
    roots.retain(|root| !root.important);
    Some(roots)
}

/// Opens the dialog of the closest npc on demand
fn show_closest_npc_dialog(
    keys: Res<ButtonInput<KeyCode>>,
    mut dialog_ui: ResMut<DialogUiState>,
    mut camera_info: ResMut<CameraResource>,
    gothic_script: Res<ScriptResource>,
    mut npcs: Query<
        (Entity, &GameNpc, &mut Transform),
        (With<GameNpc>, Without<PlayerPhysicsBodyMarker>),
    >,
    mut player: Query<&mut Transform, With<PlayerPhysicsBodyMarker>>,
    mut player_npc: Query<&GameNpc, With<PlayerCharacter>>,
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

    let Ok(mut player_tr) = player.single_mut() else {
        return;
    };
    let Ok(player_npc) = player_npc.single_mut() else {
        return;
    };
    let player_pos = player_tr.translation;

    // TODO use spatial query to get closest entity, spatial query own or from physics
    let Some((closest_entity, closest_npc, mut tr)) = npcs
        .iter_mut()
        .filter(|(_, _npc, tr)| tr.translation.distance(player_pos) < 5.0)
        .min_by(|a, b| {
            let a_d = a.2.translation.distance_squared(player_pos);
            let b_d = b.2.translation.distance_squared(player_pos);
            a_d.total_cmp(&b_d)
        })
    else {
        println!("No npc closer than 5 meters");
        return;
    };

    println!("Closest npc: {}", closest_npc.npc_instance_id);

    let mut gothic_script = gothic_script.lock();
    let npc_name = gothic_script.call_get_name(closest_npc.npc_instance_id.symbol());

    let Some(dialog_roots) = npc_dialog_roots(
        &mut gothic_script,
        player_npc.npc_instance_id,
        closest_npc.npc_instance_id,
    ) else {
        println!(
            "No dialog_roots for npc({}), player({})",
            closest_npc.npc_instance_id, player_npc.npc_instance_id
        );
        return;
    };

    face_towards(&mut tr, player_tr.translation);
    face_towards(&mut player_tr, tr.translation);

    camera_info.set_dialog_camera(*tr);

    dialog_ui.open(
        closest_entity,
        player_npc.npc_instance_id,
        closest_npc.npc_instance_id,
        dialog_roots,
        npc_name,
    );
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
