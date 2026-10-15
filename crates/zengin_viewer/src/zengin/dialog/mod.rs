mod state;
mod ui;

use crate::gameplay::camera::{CameraResource, CurrentCamera};
use crate::gameplay::character::PlayerPhysicsBodyMarker;
use crate::gameplay::objects::PlayerCharacter;
use crate::zengin::GameNpc;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::manager::ScriptManager;
use crate::zengin::script::state::DialogRoot;
use crate::zengin::script::symbol_id::InstanceId;
use crate::zengin::selection::{Selection, SelectionKind};
use bevy::log::{info, warn};
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
        .npc_instances
        .get(&npc_talk_to)?
        .dialog_roots
        .clone()?;
    gothic_script.filter_dialog_roots(npc_player, npc_talk_to, &mut roots);
    roots.retain(|root| !root.important);
    Some(roots)
}

/// Opens the dialog of the currently selected npc on demand
#[allow(clippy::type_complexity)]
fn show_closest_npc_dialog(
    keys: Res<ButtonInput<KeyCode>>,
    selection: Res<Selection>,
    mut dialog_ui: ResMut<DialogUiState>,
    mut camera_info: ResMut<CameraResource>,
    gothic_script: Res<ScriptResource>,
    mut npcs: Query<(&GameNpc, &mut Transform), Without<PlayerPhysicsBodyMarker>>,
    mut player: Query<&mut Transform, With<PlayerPhysicsBodyMarker>>,
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

    let Ok(mut player_tr) = player.single_mut() else {
        return;
    };
    let Ok(player_npc) = player_npc.single() else {
        return;
    };
    let Ok((closest_npc, mut tr)) = npcs.get_mut(closest_entity) else {
        return;
    };

    info!("Selected npc: {}", closest_npc.npc_instance_id);

    // The selection system already resolved the npc name
    let npc_name = selection.name.clone();

    let mut gothic_script = gothic_script.lock();
    let Some(dialog_roots) = npc_dialog_roots(
        &mut gothic_script,
        player_npc.npc_instance_id,
        closest_npc.npc_instance_id,
    ) else {
        warn!(
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
