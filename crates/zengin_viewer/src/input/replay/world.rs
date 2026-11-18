use bevy::prelude::*;

use crate::gameplay::objects::PlayerCharacter;
use crate::zengin::{GameNpc, ScriptResource, WeaponKind};

/// Builds a quaternion from degrees labeled by axis:
/// `(x = pitch, y = yaw, z = roll)`, applied as yaw around Y first, then
/// pitch around X, then roll around Z (the standard camera convention).
pub(super) fn rotation_from_degrees(rotation: Vec3) -> Quat {
    Quat::from_euler(
        EulerRot::YXZ,
        rotation.y.to_radians(),
        rotation.x.to_radians(),
        rotation.z.to_radians(),
    )
}

/// Teleports the player entity to `translation` (model root/feet) with
/// `rotation` in axis-labeled degrees `(x = pitch, y = yaw, z = roll)`.
pub(super) fn teleport_player(
    player: &mut Query<&mut Transform, (With<PlayerCharacter>, Without<Camera3d>)>,
    translation: Vec3,
    rotation: Vec3,
) {
    let Ok(mut transform) = player.single_mut() else {
        warn!("Replay teleport failed: no player character found");
        return;
    };
    transform.translation = translation;
    transform.rotation = rotation_from_degrees(rotation);
}

/// Sets the camera to `translation` with `rotation` in axis labeled degrees
/// `(x = pitch, y = yaw, z = roll)`.
pub(super) fn set_camera_location(
    camera: &mut Query<&mut Transform, (With<Camera3d>, Without<PlayerCharacter>)>,
    translation: Vec3,
    rotation: Vec3,
) {
    let Ok(mut transform) = camera.single_mut() else {
        warn!("Replay set camera location failed: no camera found");
        return;
    };
    transform.translation = translation;
    transform.rotation = rotation_from_degrees(rotation);
}

/// Equips the named item on the player as a drawn weapon. Item names are
/// matched case-insensitively against the loaded item instance symbols.
pub(super) fn equip_player_weapon(
    gothic_script: &ScriptResource,
    player_npc: &Query<&GameNpc, With<PlayerCharacter>>,
    item: &str,
) {
    let Ok(player_npc) = player_npc.single() else {
        warn!("Replay equip failed: no player character found");
        return;
    };
    let player_id = player_npc.instance_id();

    let mut script = gothic_script.lock();
    let lowered = item.to_lowercase();
    let Some(item_id) = script
        .get_loaded_state()
        .items_instances
        .iter()
        .find(|(_, instance)| {
            instance
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .name
                .to_lowercase()
                == lowered
        })
        .map(|(id, _)| *id)
    else {
        warn!("Replay equip failed: no item instance named {item}");
        return;
    };

    let ranged = WeaponKind::from_symbol_name(&lowered).is_some_and(|kind| kind.is_ranged());
    script.equip_item(player_id, item_id);
    script.draw_weapon(player_id, ranged);
    info!(
        "Replay equipped item({item}) on player({}) as drawn weapon",
        player_id.value()
    );
}

#[allow(clippy::cognitive_complexity)]
pub(super) fn toggle_player_armor(
    gothic_script: &ScriptResource,
    player_npc: &Query<&GameNpc, With<PlayerCharacter>>,
    item: &str,
) {
    let Ok(player_npc) = player_npc.single() else {
        warn!("Replay armor toggle failed: no player character found");
        return;
    };
    let player_id = player_npc.instance_id();

    let mut script = gothic_script.lock();
    let lowered = item.to_lowercase();
    let Some(item_id) = script
        .get_loaded_state()
        .items_instances
        .iter()
        .find(|(_, instance)| {
            instance
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .name
                .to_lowercase()
                == lowered
        })
        .map(|(id, _)| *id)
    else {
        warn!("Replay armor toggle failed: no item instance named {item}");
        return;
    };

    let equipped = script.npc_state(player_id).map(|npc| npc.equipment.armor);
    if equipped == Some(Some(item_id)) {
        script.unequip_armor(player_id);
        info!(
            "Replay removed armor({item}) from player({})",
            player_id.value()
        );
    } else {
        script.equip_item(player_id, item_id);
        info!(
            "Replay equipped armor({item}) on player({})",
            player_id.value()
        );
    }
}
