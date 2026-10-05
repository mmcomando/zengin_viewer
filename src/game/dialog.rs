use crate::game::objects::GameNpc;
use crate::zengin::PlayerMarker;
use crate::zengin::script::component::WorldScript;
use bevy::prelude::*;

pub struct GameDialogPlugin;

impl Plugin for GameDialogPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, show_closest_npc_dialog);
    }
}

fn show_closest_npc_dialog(
    keys: Res<ButtonInput<KeyCode>>,
    mut gothic_script: ResMut<WorldScript>,
    npcs: Query<(&GameNpc, &GlobalTransform)>,
    player: Query<&GlobalTransform, With<PlayerMarker>>,
) {
    // TODO action instead of key
    if !keys.just_pressed(KeyCode::KeyE) {
        return;
    }

    let Ok(player_tr) = player.single() else {
        return;
    };
    let player_pos = player_tr.translation();

    // TODO use spatial query to get closest entity, spatial query own or from physics
    let Some((closest_npc, _)) = npcs
        .iter()
        .filter(|(npc, tr)| {
            // TODO better way to discriminate player
            tr.translation().distance(player_pos) < 5.0 && npc.npc_intance_id != 1620
        })
        .min_by(|a, b| {
            let a_d = a.1.translation().distance_squared(player_pos);
            let b_d = b.1.translation().distance_squared(player_pos);
            a_d.total_cmp(&b_d)
        })
    else {
        println!("No npc closer than 5 meters");
        return;
    };

    println!("Closest npc: {}", closest_npc.npc_intance_id);

    let loaded_state = gothic_script.get_loaded_state();
    let Some(dialog_roots) = loaded_state
        .dialog_roots
        .get(&(closest_npc.npc_intance_id as u32))
    else {
        println!("No dialog_roots for npc({})", closest_npc.npc_intance_id);
        return;
    };

    println!("dialog_roots for npc({}):", closest_npc.npc_intance_id);
    for dialog_root in dialog_roots {
        println!(
            "  DialogRoot: func_id({}), description({})",
            dialog_root.sequence_function_id, dialog_root.description
        );
    }

    let dialog_roots = dialog_roots.clone();

    gothic_script.clear_dialog_lines();

    for dialog_root in dialog_roots {
        gothic_script.call_dialog_sequence_function(dialog_root.sequence_function_id);
    }

    println!("dialog lines after executing dialog_roots:");
    for dialog_line in gothic_script.dialog_lines() {
        println!(
            "  DialogLine: npc_a({}) npc_b({}) line_id({})",
            dialog_line.npc_a, dialog_line.npc_b, dialog_line.line_id
        );
    }
}
