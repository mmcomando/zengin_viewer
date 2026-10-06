mod state;
mod ui;

use crate::game::objects::GameNpc;
use crate::zengin::PlayerMarker;
use crate::zengin::script::component::WorldScript;
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

/// Opens the dialog of the closest npc on demand
fn show_closest_npc_dialog(
    keys: Res<ButtonInput<KeyCode>>,
    mut dialog_ui: ResMut<DialogUiState>,
    mut gothic_script: ResMut<WorldScript>,
    npcs: Query<(Entity, &GameNpc, &GlobalTransform)>,
    player: Query<&GlobalTransform, With<PlayerMarker>>,
) {
    // TODO action instead of key
    if !keys.just_pressed(KeyCode::KeyE) {
        return;
    }

    // Dialog already open, it stays open until EXIT is selected
    if dialog_ui.is_open() {
        return;
    }

    let Ok(player_tr) = player.single() else {
        return;
    };
    let player_pos = player_tr.translation();

    // TODO use spatial query to get closest entity, spatial query own or from physics
    let Some((closest_entity, closest_npc, _)) = npcs
        .iter()
        .filter(|(_, npc, tr)| {
            // TODO better way to discriminate player
            tr.translation().distance(player_pos) < 5.0 && npc.npc_intance_id != 1620
        })
        .min_by(|a, b| {
            let a_d = a.2.translation().distance_squared(player_pos);
            let b_d = b.2.translation().distance_squared(player_pos);
            a_d.total_cmp(&b_d)
        })
    else {
        println!("No npc closer than 5 meters");
        return;
    };

    println!("Closest npc: {}", closest_npc.npc_intance_id);

    let npc_name = gothic_script.call_get_name(closest_npc.npc_intance_id as u32);

    let loaded_state = gothic_script.get_loaded_state();
    let Some(dialog_roots) = loaded_state
        .dialog_roots
        .get(&(closest_npc.npc_intance_id as u32))
    else {
        println!("No dialog_roots for npc({})", closest_npc.npc_intance_id);
        return;
    };

    dialog_ui.open(closest_entity, dialog_roots.clone(), npc_name);
}
