//! FAI driven melee swings of an npc attack action.
//!
//! The attack pool comes from the npc's `fight_tactic` table: one move is
//! picked, expanded into the engine's swing trace and played; when the trace is
//! used up the next move is picked.

use std::collections::VecDeque;

use bevy::prelude::*;
use daedalus_wasm::daedalus::FightAiMove;
use rand_mt::Mt;

use crate::graphics::{
    AnimationController, AnimationEnds, AutoAnimSequencer, DesiredAnimation, MeleeAttackKind,
    SetDesiredAnimation, is_melee_attack_animation,
};
use crate::zengin::common::horizontal_offset;
use crate::zengin::npc_actions::action::ExecutingActionMarker;
use crate::zengin::npc_routines::{
    AnimationFiles, NpcHealth, NpcOverlay, queue_root_motion_animation, queue_stand_animation,
};
use crate::zengin::resources::CachedAssetHandles;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::WeaponMode;
use crate::zengin::script::symbol_id::InstanceId;
use crate::zengin::{FightAiResource, GameNpc};

/// Horizontal distance within which an attacking npc stops chasing and swings.
const ATTACK_RANGE: f32 = 1.0;

/// Horizontal distance beyond which an attack action is abandoned.
const ATTACK_MAX_DISTANCE: f32 = 100.0;

/// Chases and melees the target entity.
#[derive(Component)]
#[require(ExecutingActionMarker)]
pub(crate) struct AttackAction {
    pub(crate) target: Entity,
    pub(crate) swings: VecDeque<MeleeAttackKind>,
    pub(crate) last_move: Option<FightAiMove>,
    rng: Mt,
    rng_seeded: bool,
}

impl AttackAction {
    pub(crate) fn new(target: Entity) -> Self {
        Self {
            target,
            swings: VecDeque::new(),
            last_move: None,
            rng: Mt::default(),
            rng_seeded: false,
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn start_attack_action(
    mut commands: Commands,
    gothic_script: Res<ScriptResource>,
    fight_ai: Res<FightAiResource>,
    targets: Query<(&GlobalTransform, Option<&NpcHealth>)>,
    mut npcs: Query<
        (
            Entity,
            &GameNpc,
            &mut AttackAction,
            &Transform,
            &mut AnimationController,
        ),
        Added<AttackAction>,
    >,
) {
    for (entity, npc, mut action, transform, mut controller) in &mut npcs {
        if !target_is_valid(npc.instance_id(), action.target, transform, &targets) {
            commands.entity(entity).remove::<ExecutingActionMarker>();
            continue;
        }
        draw_melee_weapon(&gothic_script, npc.instance_id());
        let fai_move = begin_attack_move(
            &mut action,
            npc.instance_id(),
            entity,
            &fight_ai,
            &gothic_script,
        );
        debug!(
            "npc({}) attack starts FAI move({fai_move:?})",
            npc.instance_id(),
        );
        controller.set_root_motion(true);
        commands.entity(entity).insert(AutoAnimSequencer::default());
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn run_attack_action(
    mut commands: Commands,
    targets: Query<(&GlobalTransform, Option<&NpcHealth>)>,
    mut desired: MessageWriter<SetDesiredAnimation>,
    mut npcs: Query<(
        Entity,
        &GameNpc,
        &mut AttackAction,
        &Transform,
        Option<&AutoAnimSequencer>,
        Option<&mut AnimationController>,
    )>,
) {
    for (entity, npc, action, transform, sequencer, controller) in &mut npcs {
        if !target_is_valid(npc.instance_id(), action.target, transform, &targets) {
            commands.entity(entity).remove::<ExecutingActionMarker>();
            continue;
        }

        let Some(mut controller) = controller else {
            crate::warn_once!(
                "npc({}) attack waits for its animation controller (visual not spawned)",
                npc.instance_id()
            );
            continue;
        };

        let target_tr = targets
            .get(action.target)
            .map(|(target_tr, _)| target_tr)
            .ok();
        let Some(target_tr) = target_tr else {
            continue;
        };
        let horizontal = horizontal_offset(transform.translation, target_tr.translation());
        let distance = horizontal.length();
        let direction = horizontal.normalize_or_zero();
        let direction = (direction.length_squared() > 0.0).then_some(direction);
        let swing = action
            .swings
            .front()
            .copied()
            .unwrap_or(MeleeAttackKind::Default);
        let animation = if distance <= ATTACK_RANGE {
            DesiredAnimation::MeleeAttack(swing)
        } else {
            DesiredAnimation::Run
        };
        trace!(
            "npc({}) attack desires {animation:?} at {distance:.2}m",
            npc.instance_id()
        );
        queue_root_motion_animation(
            &mut commands,
            &mut desired,
            entity,
            &mut controller,
            sequencer,
            animation,
            direction,
        );
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn end_attack_action(
    mut removed: RemovedComponents<AttackAction>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut npcs: Query<(
        Option<&mut AutoAnimSequencer>,
        Option<&mut AnimationController>,
        &crate::gameplay::objects::NpcModel,
        Option<&NpcOverlay>,
    )>,
) {
    for entity in removed.read() {
        let Ok((mut sequencer, controller, model, overlay)) = npcs.get_mut(entity) else {
            continue;
        };
        let Some(mut controller) = controller else {
            continue;
        };
        controller.set_root_motion(false);
        queue_stand_animation(
            &mut controller,
            sequencer.as_deref_mut(),
            model,
            overlay,
            &files,
            &mut handles,
            &asset_server,
        );
    }
}

pub(crate) fn advance_npc_attack_swings(
    mut ends: MessageReader<AnimationEnds>,
    gothic_script: Res<ScriptResource>,
    fight_ai: Res<FightAiResource>,
    mut npcs: Query<(
        Entity,
        &GameNpc,
        &mut AttackAction,
        &AnimationController,
        &mut AutoAnimSequencer,
    )>,
) {
    for end in ends.read() {
        if !is_melee_attack_animation(&end.what_animation) {
            continue;
        }
        let Ok((entity, npc, mut action, controller, mut sequencer)) = npcs.get_mut(end.who) else {
            continue;
        };
        if controller.current() != &end.what_animation {
            continue;
        }

        action.swings.pop_front();
        if action.swings.is_empty() {
            let Some(fai_move) = begin_attack_move(
                &mut action,
                npc.instance_id(),
                entity,
                &fight_ai,
                &gothic_script,
            ) else {
                continue;
            };
            debug!("npc({}) next attack move({fai_move:?})", npc.instance_id());
        }

        let Some(swing) = action.swings.front().copied() else {
            continue;
        };
        sequencer.set_desired_now(DesiredAnimation::MeleeAttack(swing));
    }
}

fn target_is_valid(
    instance: InstanceId,
    target: Entity,
    transform: &Transform,
    targets: &Query<(&GlobalTransform, Option<&NpcHealth>)>,
) -> bool {
    let Ok((target_tr, target_health)) = targets.get(target) else {
        warn!("npc({instance}) attack target({target}) is gone, dropping action");
        return false;
    };
    if target_health.is_some_and(|health| health.hitpoints <= 0) {
        warn!("npc({instance}) attack target({target}) is dead, dropping action");
        return false;
    }
    let to_target = target_tr.translation() - transform.translation;
    let horizontal = Vec3::new(to_target.x, 0.0, to_target.z);
    let distance = horizontal.length();
    if distance > ATTACK_MAX_DISTANCE {
        warn!(
            "npc({instance}) attack target is {distance:.1}m away (> {ATTACK_MAX_DISTANCE}m), \
             dropping action"
        );
        return false;
    }
    true
}

/// Adds the swings of a freshly picked FAI move and returns it. `None` when the
/// tactic has no attack entry.
pub(super) fn begin_attack_move(
    action: &mut AttackAction,
    instance: InstanceId,
    entity: Entity,
    fight_ai: &FightAiResource,
    gothic_script: &ScriptResource,
) -> Option<FightAiMove> {
    let tactic = gothic_script.lock().npc_fight_tactic(instance);
    let pool = tactic.map_or(&[][..], |tactic| fight_ai.0.focus_moves(tactic));
    let previous = action.last_move;
    let chosen = {
        let rng = attack_rng(action, instance, entity);
        pick_focus_move(pool, previous, rng)
    }?;
    let swings = {
        let rng = attack_rng(action, instance, entity);
        expand_fai_move(chosen, rng)
    };
    action.last_move = Some(chosen);
    action.swings.extend(swings);
    Some(chosen)
}

fn attack_rng(action: &mut AttackAction, instance: InstanceId, entity: Entity) -> &mut Mt {
    if !action.rng_seeded {
        action.rng_seeded = true;
        let seed = entity.to_bits() as u32 ^ instance.value() as u32;
        action.rng = Mt::new(seed);
    }
    &mut action.rng
}

/// Never returns the previous move when the pool has another attack; `None`
/// when the pool has no attack entry.
fn pick_focus_move(
    pool: &[FightAiMove],
    previous: Option<FightAiMove>,
    rng: &mut Mt,
) -> Option<FightAiMove> {
    let attacks: Vec<FightAiMove> = pool
        .iter()
        .copied()
        .filter(|fai_move| fai_move.is_attack())
        .collect();
    if attacks.is_empty() {
        return None;
    }
    let without_previous: Vec<FightAiMove> = attacks
        .iter()
        .copied()
        .filter(|fai_move| Some(*fai_move) != previous)
        .collect();
    let candidates = if without_previous.is_empty() {
        &attacks
    } else {
        &without_previous
    };
    let index = rng.next_u32() as usize % candidates.len();
    candidates.get(index).copied()
}

/// Non-attack moves yield no swings.
fn expand_fai_move(fai_move: FightAiMove, rng: &mut Mt) -> Vec<MeleeAttackKind> {
    use MeleeAttackKind::{Default, Left, Right};
    match fai_move {
        FightAiMove::Attack => vec![Default],
        FightAiMove::SideAttack => vec![Left, Right],
        FightAiMove::FrontAttack => vec![random_side(rng), Default],
        FightAiMove::TripleAttack => {
            if rng.next_u32().is_multiple_of(2) {
                vec![Default, Right, Left]
            } else {
                vec![Left, Right, Default]
            }
        }
        FightAiMove::WhirlAttack => vec![Left, Right, Left, Right],
        FightAiMove::MasterAttack => vec![Left, Right, Default, Default, Default, Default],
        FightAiMove::Run
        | FightAiMove::RunBack
        | FightAiMove::JumpBack
        | FightAiMove::Turn
        | FightAiMove::Strafe
        | FightAiMove::TurnToHit
        | FightAiMove::Parade
        | FightAiMove::StandUp
        | FightAiMove::Wait
        | FightAiMove::WaitLonger
        | FightAiMove::WaitExt => Vec::new(),
    }
}

fn random_side(rng: &mut Mt) -> MeleeAttackKind {
    if rng.next_u32().is_multiple_of(2) {
        MeleeAttackKind::Left
    } else {
        MeleeAttackKind::Right
    }
}

/// Draws the melee weapon of an npc when it has one sheathed, unarmed npcs keep
/// their fists.
fn draw_melee_weapon(gothic_script: &ScriptResource, npc: InstanceId) {
    let mut script = gothic_script.lock();
    let (has_melee, mode) = script.npc_state(npc).map_or((false, None), |npc| {
        (npc.equipment.melee.is_some(), npc.equipment.weapon_mode)
    });
    if !has_melee {
        debug!("npc({npc}) has no melee weapon, attacking unarmed");
        return;
    }
    if mode == Some(WeaponMode::Melee) {
        trace!("npc({npc}) melee weapon already drawn");
        return;
    }
    info!("npc({npc}) drawing its melee weapon for the attack");
    script.draw_weapon(npc, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rng() -> Mt {
        Mt::new(0x9E37_79B9)
    }

    #[test]
    fn attack_moves_expand_into_the_engine_swing_traces() {
        let mut rng = rng();
        assert_eq!(
            expand_fai_move(FightAiMove::Attack, &mut rng),
            [MeleeAttackKind::Default]
        );
        assert_eq!(
            expand_fai_move(FightAiMove::SideAttack, &mut rng),
            [MeleeAttackKind::Left, MeleeAttackKind::Right]
        );
        assert_eq!(
            expand_fai_move(FightAiMove::WhirlAttack, &mut rng),
            [
                MeleeAttackKind::Left,
                MeleeAttackKind::Right,
                MeleeAttackKind::Left,
                MeleeAttackKind::Right
            ]
        );
        assert_eq!(
            expand_fai_move(FightAiMove::MasterAttack, &mut rng),
            [
                MeleeAttackKind::Left,
                MeleeAttackKind::Right,
                MeleeAttackKind::Default,
                MeleeAttackKind::Default,
                MeleeAttackKind::Default,
                MeleeAttackKind::Default
            ]
        );
    }

    #[test]
    fn front_attack_starts_with_a_side_swing() {
        let mut rng = rng();
        let trace = expand_fai_move(FightAiMove::FrontAttack, &mut rng);

        assert_eq!(trace.len(), 2);
        assert!(matches!(
            trace[0],
            MeleeAttackKind::Left | MeleeAttackKind::Right
        ));
        assert_eq!(trace[1], MeleeAttackKind::Default);
    }

    #[test]
    fn triple_attack_uses_one_of_the_two_combos() {
        let mut rng = rng();
        let trace = expand_fai_move(FightAiMove::TripleAttack, &mut rng);

        let left_first = [
            MeleeAttackKind::Left,
            MeleeAttackKind::Right,
            MeleeAttackKind::Default,
        ];
        let default_first = [
            MeleeAttackKind::Default,
            MeleeAttackKind::Right,
            MeleeAttackKind::Left,
        ];
        assert!(trace == left_first || trace == default_first);
    }

    #[test]
    fn non_attack_move_has_no_swings() {
        let mut rng = rng();

        assert!(expand_fai_move(FightAiMove::Wait, &mut rng).is_empty());
    }

    #[test]
    fn pool_without_attacks_yields_no_move() {
        let mut rng = rng();

        assert_eq!(pick_focus_move(&[], None, &mut rng), None);
        assert_eq!(
            pick_focus_move(&[FightAiMove::Wait, FightAiMove::Strafe], None, &mut rng),
            None
        );
    }

    #[test]
    fn previous_move_is_avoided_when_possible() {
        let mut rng = rng();
        let pool = [FightAiMove::Attack, FightAiMove::FrontAttack];

        for _ in 0..16 {
            assert_eq!(
                pick_focus_move(&pool, Some(FightAiMove::Attack), &mut rng),
                Some(FightAiMove::FrontAttack)
            );
        }
    }

    #[test]
    fn single_attack_move_may_repeat() {
        let mut rng = rng();

        assert_eq!(
            pick_focus_move(&[FightAiMove::Attack], Some(FightAiMove::Attack), &mut rng),
            Some(FightAiMove::Attack)
        );
    }
}
