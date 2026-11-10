//! Fighter tactic tables from `FIGHT.DAT`.
//!
//! In a retail install `FIGHT.DAT` carries the instantiated `C_FIGHTAI`
//! tables: one instance per situation and tactic (`FA_MY_W_FOCUS_<tactic>`),
//! each filling its `move[6]` array in the instance-init bytecode. The
//! instance init only pushes constants and writes them into the member array,
//! so the tables are read without a second script vm.

use std::io;
use std::path::Path;

use crate::daedalus::{CompiledDaedalusScript, Instruction, Symbol, SymbolKind};

/// Number of move slots of `C_FIGHTAI.MOVE`.
pub const FIGHT_AI_MOVES: usize = 6;

/// Name of the `C_FIGHTAI` member array filled by the instance init.
const MOVE_MEMBER_NAME: &str = "c_fightai.move";
/// Name of the constant holding the number of tactic tables.
const MAX_FIGHT_AI_NAME: &str = "max_fightai";
/// Prefix of the per-tactic instance the move pool is read from.
const FOCUS_PREFIX: &str = "fa_my_w_focus_";

/// One `C_FIGHTAI.MOVE` entry, numbered like the `MOVE_*` script constants.
#[expect(clippy::exhaustive_enums, reason = "MOVE_* constants are fixed")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FightAiMove {
    /// `MOVE_RUN`: run towards the opponent.
    Run,
    /// `MOVE_RUNBACK`: run backwards.
    RunBack,
    /// `MOVE_JUMPBACK`: jump backwards.
    JumpBack,
    /// `MOVE_TURN`: turn towards the opponent.
    Turn,
    /// `MOVE_STRAFE`: strafe to the side.
    Strafe,
    /// `MOVE_ATTACK`: the weapon's default attack.
    Attack,
    /// `MOVE_SIDEATTACK`: left, then right swing.
    SideAttack,
    /// `MOVE_FRONTATTACK`: a random side swing followed by the default one.
    FrontAttack,
    /// `MOVE_TRIPLEATTACK`: three swing combo.
    TripleAttack,
    /// `MOVE_WHIRLATTACK`: four swing combo.
    WhirlAttack,
    /// `MOVE_MASTERATTACK`: six swing combo.
    MasterAttack,
    /// `MOVE_TURNTOHIT`: turn into the hit.
    TurnToHit,
    /// `MOVE_PARADE`: block.
    Parade,
    /// `MOVE_STANDUP`: get up.
    StandUp,
    /// `MOVE_WAIT`: wait.
    Wait,
    /// `MOVE_WAIT_LONGER`: wait longer.
    WaitLonger,
    /// `MOVE_WAIT_EXT`: extended wait.
    WaitExt,
}

impl FightAiMove {
    /// Move for a raw `C_FIGHTAI.MOVE` value, `None` for the unset slot (`0`)
    /// and for values this build does not know.
    #[must_use]
    pub fn from_value(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Run),
            2 => Some(Self::RunBack),
            3 => Some(Self::JumpBack),
            4 => Some(Self::Turn),
            5 => Some(Self::Strafe),
            6 => Some(Self::Attack),
            7 => Some(Self::SideAttack),
            8 => Some(Self::FrontAttack),
            9 => Some(Self::TripleAttack),
            10 => Some(Self::WhirlAttack),
            11 => Some(Self::MasterAttack),
            15 => Some(Self::TurnToHit),
            17 => Some(Self::Parade),
            18 => Some(Self::StandUp),
            19 => Some(Self::Wait),
            23 => Some(Self::WaitLonger),
            24 => Some(Self::WaitExt),
            _ => None,
        }
    }

    /// Whether the move is a melee attack with a swing trace.
    #[must_use]
    pub fn is_attack(self) -> bool {
        matches!(
            self,
            Self::Attack
                | Self::SideAttack
                | Self::FrontAttack
                | Self::TripleAttack
                | Self::WhirlAttack
                | Self::MasterAttack
        )
    }
}

/// Focused-enemy attack pools indexed by `C_Npc.fight_tactic`.
#[derive(Debug, Default, Clone)]
pub struct FightAiTables {
    focus: Vec<Vec<FightAiMove>>,
}

impl FightAiTables {
    /// Reads the tables from a compiled `FIGHT.DAT`.
    ///
    /// The file needs the `C_FIGHTAI` class with its `MOVE` member; without it
    /// the data is not a fighter table and the call fails.
    pub fn load(path: &Path) -> io::Result<Self> {
        let script = CompiledDaedalusScript::from_file(path)?;
        let Some(move_member) = script
            .symbols
            .iter()
            .position(|symbol| symbol.name == MOVE_MEMBER_NAME)
        else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} has no {MOVE_MEMBER_NAME} symbol", path.display()),
            ));
        };

        let mut focus = vec![Vec::new(); tactic_count(&script)];
        for symbol in &script.symbols {
            if symbol.kind != SymbolKind::Instance {
                continue;
            }
            let Some(tactic) = focus_tactic(&symbol.name) else {
                continue;
            };
            let Some(moves) = instance_moves(&script.symbols, symbol, move_member as u32) else {
                continue;
            };
            let moves = moves
                .iter()
                .map_while(|value| FightAiMove::from_value(*value))
                .collect();
            if let Some(slot) = focus.get_mut(tactic) {
                *slot = moves;
            }
        }

        Ok(Self { focus })
    }

    /// Focused-enemy attack pool of a tactic, empty for unknown tactics and
    /// for tactics whose table has no entries.
    #[must_use]
    pub fn focus_moves(&self, tactic: i32) -> &[FightAiMove] {
        usize::try_from(tactic)
            .ok()
            .and_then(|tactic| self.focus.get(tactic))
            .map_or(&[], Vec::as_slice)
    }
}

/// Number of tactic tables, from `MAX_FIGHTAI` widened to the highest
/// instantiated tactic so a missing constant does not drop data.
fn tactic_count(script: &CompiledDaedalusScript) -> usize {
    let declared = script
        .symbols
        .iter()
        .find(|symbol| symbol.name == MAX_FIGHT_AI_NAME)
        .and_then(|symbol| symbol.default_values.first().copied())
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_default();
    let instantiated = script
        .symbols
        .iter()
        .filter_map(|symbol| focus_tactic(&symbol.name))
        .max()
        .map_or(0, |tactic| tactic + 1);
    declared.max(instantiated)
}

/// Tactic index of a `FA_MY_W_FOCUS_<tactic>` instance name.
fn focus_tactic(name: &str) -> Option<usize> {
    let tactic = name.strip_prefix(FOCUS_PREFIX)?;
    tactic.parse().ok()
}

/// Values of the instance's `C_FIGHTAI.MOVE` array, read from its init code.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "Only the instance-init instruction subset is interpreted"
)]
fn instance_moves(
    symbols: &[Symbol],
    instance: &Symbol,
    move_member: u32,
) -> Option<[i32; FIGHT_AI_MOVES]> {
    let function = instance.function.as_ref()?;
    let mut moves = [0; FIGHT_AI_MOVES];
    let mut stack: Vec<InitValue> = Vec::new();
    let mut current_instance = instance.index;

    for instruction in &function.instructions {
        match *instruction {
            Instruction::PushInt(value) => stack.push(InitValue::Int(value)),
            Instruction::PushVar(symbol) => {
                let data = symbols.get(symbol as usize)?;
                if data.is_class_var {
                    stack.push(InitValue::Reference { symbol, element: 0 });
                } else {
                    stack.push(InitValue::Int(*data.default_values.first()?));
                }
            }
            Instruction::PushArrayVar(symbol, element) => stack.push(InitValue::Reference {
                symbol,
                element: element as usize,
            }),
            Instruction::SetInstance(symbol) => current_instance = symbol,
            Instruction::AssignInteger => {
                let (reference, value) = (stack.pop()?, stack.pop()?);
                let InitValue::Reference { symbol, element } = reference else {
                    return None;
                };
                let InitValue::Int(value) = value else {
                    return None;
                };
                if symbol == move_member && current_instance == instance.index {
                    *moves.get_mut(element)? = value;
                }
            }
            Instruction::Return => break,
            _ => return None,
        }
    }

    Some(moves)
}

/// Stack entry of the instance-init interpreter.
#[derive(Debug, Clone, Copy)]
enum InitValue {
    Int(i32),
    Reference { symbol: u32, element: usize },
}
