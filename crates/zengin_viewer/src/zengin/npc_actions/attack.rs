//! FAI driven melee swings of an npc attack action.
//!
//! The attack pool comes from the npc's `fight_tactic` table: one move is
//! picked, expanded into the engine's swing trace and played; when the trace is
//! used up the next move is picked.

use bevy::prelude::*;
use daedalus_wasm::daedalus::FightAiMove;
use rand_mt::Mt;

use crate::graphics::{
    AnimationController, AnimationEnds, AutoAnimSequencer, DesiredAnimation, MeleeAttackKind,
    is_melee_attack_animation,
};
use crate::zengin::npc_actions::queue::{NpcAction, NpcActionQueue};
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::symbol_id::InstanceId;
use crate::zengin::{FightAiResource, GameNpc};

pub(crate) fn advance_npc_attack_swings(
    mut ends: MessageReader<AnimationEnds>,
    gothic_script: Res<ScriptResource>,
    fight_ai: Res<FightAiResource>,
    mut npcs: Query<(
        Entity,
        &GameNpc,
        &mut NpcActionQueue,
        &AnimationController,
        &mut AutoAnimSequencer,
    )>,
) {
    for end in ends.read() {
        if !is_melee_attack_animation(&end.what_animation) {
            continue;
        }
        let Ok((entity, npc, mut queue, controller, mut sequencer)) = npcs.get_mut(end.who) else {
            continue;
        };
        if !matches!(queue.front(), Some(NpcAction::Attack { .. })) {
            continue;
        }
        if controller.current() != &end.what_animation {
            continue;
        }

        queue.attack_mut().swings.pop_front();
        if queue.attack().swings.is_empty() {
            let Some(fai_move) = begin_attack_move(
                &mut queue,
                npc.instance_id(),
                entity,
                &fight_ai,
                &gothic_script,
            ) else {
                continue;
            };
            debug!("npc({}) next attack move({fai_move:?})", npc.instance_id());
        }

        let Some(swing) = queue.attack().swings.front().copied() else {
            continue;
        };
        sequencer.set_desired_now(DesiredAnimation::MeleeAttack(swing));
    }
}

/// Adds the swings of a freshly picked FAI move and returns it. `None` when the
/// tactic has no attack entry.
pub(super) fn begin_attack_move(
    queue: &mut NpcActionQueue,
    instance: InstanceId,
    entity: Entity,
    fight_ai: &FightAiResource,
    gothic_script: &ScriptResource,
) -> Option<FightAiMove> {
    let tactic = gothic_script.lock().npc_fight_tactic(instance);
    let pool = tactic.map_or(&[][..], |tactic| fight_ai.0.focus_moves(tactic));
    let previous = queue.attack().last_move;
    let chosen = {
        let rng = queue.attack_rng(instance, entity);
        pick_focus_move(pool, previous, rng)
    }?;
    let swings = {
        let rng = queue.attack_rng(instance, entity);
        expand_fai_move(chosen, rng)
    };
    queue.attack_mut().last_move = Some(chosen);
    queue.attack_mut().swings.extend(swings);
    Some(chosen)
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
