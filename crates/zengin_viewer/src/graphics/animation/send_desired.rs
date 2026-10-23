//! Sets the desired animation of the player from gameplay input: movement
//! (camera relative), standing and attacking (aiming at the selected npc).
//! The transition from the current to the desired animation happens in the
//! sequencer ([`super::run_auto_anim_sequencer`]).

use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::gameplay::objects::PlayerCharacter;
use crate::input::{GameplayAction, GameplayInputMarker};
use crate::zengin::{NpcWeaponMode, Selection, WeaponMode};

use super::auto_anim_sequencer::{AutoAnimSequencer, DesiredAnimation, SetDesiredAnimation};

/// Writes the desired animation of the player every frame from the move
/// input, resolved camera relative (forward input moves away from the
/// camera; holds for every camera mode). Firing (ranged weapon drawn and
/// attack pressed) wins over movement; the model aims at the target.
#[allow(clippy::type_complexity)]
pub(super) fn send_desired_animations(
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    players: Query<
        (Entity, Option<&NpcWeaponMode>, &GlobalTransform),
        (With<PlayerCharacter>, With<AutoAnimSequencer>),
    >,
    camera: Single<&Transform, With<Camera3d>>,
    selection: Res<Selection>,
    transforms: Query<&GlobalTransform>,
    mut writer: MessageWriter<SetDesiredAnimation>,
) {
    let Ok((entity, weapon_mode, player_tr)) = players.single() else {
        return;
    };

    let mode = weapon_mode.map_or(WeaponMode::Fists, |mode| mode.0);
    let attacking = match mode {
        WeaponMode::Ranged => action_state.pressed(&GameplayAction::Interact),
        WeaponMode::Melee => {
            action_state.pressed(&GameplayAction::Interact)
                || action_state.pressed(&GameplayAction::DealDamage)
        }
        WeaponMode::Fists => false,
    };

    // Quick check first: standing still is the common case, and the attack
    // branch below wins over it when attacking.
    let direction = action_state.axis_pair(&GameplayAction::Move);
    if !attacking && direction.length_squared() < 0.1 {
        set_stand_animation(entity, &mut writer);
        return;
    }

    if attacking {
        set_attack_animation(
            entity,
            mode,
            player_tr,
            &selection,
            &transforms,
            &mut writer,
        );
        return;
    }

    set_run_animation(entity, &camera, direction, &mut writer);
}

/// Writes the attack animation: melee swing or bow fire depending on the
/// drawn weapon. The model aims at the selected npc, falling back to the
/// current facing direction when nothing is selected.
fn set_attack_animation(
    entity: Entity,
    mode: WeaponMode,
    player_tr: &GlobalTransform,
    selection: &Selection,
    transforms: &Query<&GlobalTransform>,
    writer: &mut MessageWriter<SetDesiredAnimation>,
) {
    // Aim at the selected npc, fall back to the current facing direction.
    let target = selection
        .target
        .and_then(|target| transforms.get(target).ok()).map_or_else(|| player_tr.translation() + (player_tr.rotation() * Vec3::Z) * 20.0, |target| target.translation());
    let direction = horizontal(target - player_tr.translation());
    let direction = if direction.length_squared() < 0.0001 {
        None
    } else {
        Some(direction)
    };
    let animation = match mode {
        WeaponMode::Ranged => DesiredAnimation::Fire,
        _ => DesiredAnimation::MeleeAttack,
    };
    writer.write(SetDesiredAnimation {
        who: entity,
        animation,
        direction,
        force_change: true,
    });
}

/// Writes the stand (idle) animation for the player.
fn set_stand_animation(entity: Entity, writer: &mut MessageWriter<SetDesiredAnimation>) {
    writer.write(SetDesiredAnimation {
        who: entity,
        animation: DesiredAnimation::Stand,
        direction: None,
        force_change: false,
    });
}

/// Writes the run animation moving in `move_direction` (stick axes), resolved
/// relative to the camera orientation.
fn set_run_animation(
    entity: Entity,
    camera: &Transform,
    move_direction: Vec2,
    writer: &mut MessageWriter<SetDesiredAnimation>,
) {
    let forward = horizontal(*camera.forward());
    let right = horizontal(*camera.right());
    let dir = right * move_direction.x + forward * move_direction.y;
    if dir.length_squared() < 0.0001 {
        warn!("Move input above the dead zone but no horizontal camera direction");
        return;
    }
    writer.write(SetDesiredAnimation {
        who: entity,
        animation: DesiredAnimation::Run,
        direction: Some(dir.normalize()),
        force_change: true,
    });
}

/// Direction projected onto the horizontal plane.
fn horizontal(direction: Vec3) -> Vec3 {
    Vec3::new(direction.x, 0.0, direction.z).normalize_or_zero()
}
