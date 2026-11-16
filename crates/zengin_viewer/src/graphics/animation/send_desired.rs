//! Sets the desired animation of the player from gameplay input: movement
//! (camera relative), standing and attacking (aiming at the selected npc).
//! The transition from the current to the desired animation happens in the
//! sequencer ([`super::run_auto_anim_sequencer`]).

use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::gameplay::camera::{CameraResource, CurrentCamera};
use crate::gameplay::objects::PlayerCharacter;
use crate::input::{GameplayAction, GameplayInputMarker};
use crate::zengin::{NpcWeaponMode, Selection, WeaponMode};

use super::auto_anim_sequencer::{
    AutoAnimSequencer, DesiredAnimation, MeleeAttackKind, SetDesiredAnimation, TurnDirection,
    is_melee_attack_animation,
};
use super::controller::AnimationController;

/// Writes the desired animation of the player every frame from the move
/// input, resolved camera relative (forward input moves away from the camera;
/// holds for every camera mode). Sideways input takes priority and strafes
/// while the model keeps facing the camera direction. Firing (ranged weapon
/// drawn and attack pressed) wins over movement; the model aims at the target.
/// A directional attack press wins over both for one swing.
#[allow(clippy::type_complexity)]
pub(super) fn send_desired_animations(
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    players: Query<
        (
            Entity,
            Option<&NpcWeaponMode>,
            &GlobalTransform,
            Option<&AnimationController>,
        ),
        (With<PlayerCharacter>, With<AutoAnimSequencer>),
    >,
    camera: Single<&Transform, With<Camera3d>>,
    camera_info: Res<CameraResource>,
    selection: Res<Selection>,
    transforms: Query<&GlobalTransform>,
    mut writer: MessageWriter<SetDesiredAnimation>,
) {
    let Ok((entity, weapon_mode, player_tr, controller)) = players.single() else {
        return;
    };

    let mode = weapon_mode.map_or(WeaponMode::Fists, |mode| mode.0);

    // A directional press is a one-shot swing: it is only started when no
    // swing is played or queued, so mashing does not chain swings.
    if let Some(kind) = directional_attack(&action_state)
        && mode == WeaponMode::Melee
        && !controller.is_some_and(swing_is_playing)
    {
        set_attack_animation(
            entity,
            DesiredAnimation::MeleeAttack(kind),
            player_tr,
            &selection,
            &transforms,
            &mut writer,
        );
        return;
    }

    let attacking = match mode {
        WeaponMode::Ranged => action_state.pressed(&GameplayAction::Interact),
        WeaponMode::Melee => {
            action_state.pressed(&GameplayAction::Interact)
                || action_state.pressed(&GameplayAction::DealDamage)
        }
        WeaponMode::Fists => false,
    };

    let direction = action_state
        .axis_pair(&GameplayAction::Move)
        .clamp_length_max(1.0);

    // Quick check first: standing still is the common case, and the attack
    // branch below wins over it when attacking.
    if !attacking && direction.length_squared() < 0.1 {
        set_idle_facing(entity, player_tr, &camera, &camera_info, &mut writer);
        return;
    }

    if attacking {
        set_attack_animation(
            entity,
            attack_animation(mode),
            player_tr,
            &selection,
            &transforms,
            &mut writer,
        );
        return;
    }

    // Sideways input wins over forward/backward (like OpenGothic): the model
    // keeps facing the camera direction, like while running forward, and the
    // strafe loop carries it to the side.
    if direction.x.abs() >= STRAFE_DEADZONE {
        set_strafe_animation(entity, direction.x, &camera, &mut writer);
        return;
    }

    set_run_animation(entity, &camera, direction, &mut writer);
}

/// Sideways input above this magnitude makes the character strafe instead of
/// running in the input direction.
const STRAFE_DEADZONE: f32 = 0.1;

/// One-shot swing requested this frame; left wins when both keys are pressed
/// together.
fn directional_attack(action_state: &ActionState<GameplayAction>) -> Option<MeleeAttackKind> {
    if action_state.just_pressed(&GameplayAction::AttackLeft) {
        return Some(MeleeAttackKind::Left);
    }
    if action_state.just_pressed(&GameplayAction::AttackRight) {
        return Some(MeleeAttackKind::Right);
    }
    None
}

/// Whether a melee swing is played or queued, so another directional press is
/// dropped instead of chaining.
fn swing_is_playing(controller: &AnimationController) -> bool {
    is_melee_attack_animation(controller.current())
        || controller.next().is_some_and(is_melee_attack_animation)
}

/// Loop animation of the held attack key for the drawn weapon.
fn attack_animation(mode: WeaponMode) -> DesiredAnimation {
    match mode {
        WeaponMode::Ranged => DesiredAnimation::Fire,
        WeaponMode::Fists => DesiredAnimation::MeleeAttack(MeleeAttackKind::Fist),
        WeaponMode::Melee => DesiredAnimation::MeleeAttack(MeleeAttackKind::Default),
    }
}

/// Writes the attack animation. The model aims at the selected npc, falling
/// back to the current facing direction when nothing is selected.
fn set_attack_animation(
    entity: Entity,
    animation: DesiredAnimation,
    player_tr: &GlobalTransform,
    selection: &Selection,
    transforms: &Query<&GlobalTransform>,
    writer: &mut MessageWriter<SetDesiredAnimation>,
) {
    // Aim at the selected npc, fall back to the current facing direction.
    let target = selection
        .target
        .and_then(|target| transforms.get(target).ok())
        .map_or_else(
            || player_tr.translation() + (player_tr.rotation() * Vec3::Z) * 20.0,
            |target| target.translation(),
        );
    let direction = horizontal(target - player_tr.translation());
    let direction = if direction.length_squared() < 0.0001 {
        None
    } else {
        Some(direction)
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

/// Angle below which the idle model is not considered to be turning, keeps
/// the turn animation from flickering on tiny camera movement.
const TURN_ANIMATION_DEADZONE: f32 = 0.08;

/// Writes the idle facing: the character turns towards the third person camera
/// direction, playing the turn animation while the angle is large. The free and
/// dialog cameras do not drive the player's facing.
fn set_idle_facing(
    entity: Entity,
    player_tr: &GlobalTransform,
    camera: &Transform,
    camera_info: &CameraResource,
    writer: &mut MessageWriter<SetDesiredAnimation>,
) {
    if camera_info.current() != CurrentCamera::ThirdPerson {
        set_stand_animation(entity, writer);
        return;
    }
    let target = horizontal(*camera.forward());
    if target.length_squared() < 0.0001 {
        set_stand_animation(entity, writer);
        return;
    }
    // The npc model faces its local +Z, unlike the Bevy camera forward (-Z).
    let current = horizontal(player_tr.rotation() * Vec3::Z);
    let angle = signed_yaw_between(current, target);
    let (animation, force_change) = if angle.abs() > TURN_ANIMATION_DEADZONE {
        (DesiredAnimation::Turn(turn_direction(angle)), true)
    } else {
        (DesiredAnimation::Stand, false)
    };
    writer.write(SetDesiredAnimation {
        who: entity,
        animation,
        direction: Some(target),
        force_change,
    });
}

/// Signed horizontal angle from `from` towards `to`, positive when the model
/// turns to its left.
fn signed_yaw_between(from: Vec3, to: Vec3) -> f32 {
    let cross_y = from.z * to.x - from.x * to.z;
    cross_y.atan2(from.dot(to).clamp(-1.0, 1.0))
}

fn turn_direction(angle: f32) -> TurnDirection {
    if angle > 0.0 {
        TurnDirection::Left
    } else {
        TurnDirection::Right
    }
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

/// Writes the sideways run loop matching `side` (negative is left). The model
/// faces the camera direction, like while running forward, so turning the
/// camera turns the character and the strafe root motion stays relative to it.
fn set_strafe_animation(
    entity: Entity,
    side: f32,
    camera: &Transform,
    writer: &mut MessageWriter<SetDesiredAnimation>,
) {
    let animation = if side < 0.0 {
        DesiredAnimation::StrafeLeft
    } else {
        DesiredAnimation::StrafeRight
    };
    let forward = horizontal(*camera.forward());
    let direction = if forward.length_squared() < 0.0001 {
        warn!("Strafe input but no horizontal camera direction");
        None
    } else {
        Some(forward)
    };
    writer.write(SetDesiredAnimation {
        who: entity,
        animation,
        direction,
        force_change: true,
    });
}

/// Direction projected onto the horizontal plane.
fn horizontal(direction: Vec3) -> Vec3 {
    Vec3::new(direction.x, 0.0, direction.z).normalize_or_zero()
}
