//! `FIGHT.DAT` fighter table reading against the local game data.

use std::path::PathBuf;

use crate::daedalus::{FightAiMove, FightAiTables};
use crate::tests::common::gothic2_dir;

fn fight_ai_path() -> PathBuf {
    gothic2_dir().join("_work/Data/Scripts/_compiled/FIGHT.DAT")
}

#[test]
fn reads_known_human_tactics() {
    let tables = FightAiTables::load(&fight_ai_path()).expect("FIGHT.DAT readable");

    assert_eq!(tables.focus_moves(42), [FightAiMove::Attack]);
    assert_eq!(
        tables.focus_moves(4),
        [
            FightAiMove::FrontAttack,
            FightAiMove::Attack,
            FightAiMove::Wait,
        ]
    );
}

#[test]
fn unknown_tactic_has_no_moves() {
    let tables = FightAiTables::load(&fight_ai_path()).expect("FIGHT.DAT readable");

    assert!(tables.focus_moves(999).is_empty());
    assert!(tables.focus_moves(-1).is_empty());
}
