//! G2 damage and protection array layout shared by `C_Npc` and `C_Item`.

/// Number of `C_Npc.damage`, `C_Npc.protection` and `C_Item.damage`
/// slots.
pub const DAMAGE_TYPES: usize = 8;

/// `DAM_INVALID`, the "no damage type" bitmask.
pub const DAM_INVALID: i32 = 0;
/// `DAM_BARRIER` bit.
pub const DAM_BARRIER: i32 = 1 << 0;
/// `DAM_BLUNT` bit.
pub const DAM_BLUNT: i32 = 1 << 1;
/// `DAM_EDGE` bit.
pub const DAM_EDGE: i32 = 1 << 2;
/// `DAM_FIRE` bit.
pub const DAM_FIRE: i32 = 1 << 3;
/// `DAM_FLY` bit.
pub const DAM_FLY: i32 = 1 << 4;
/// `DAM_MAGIC` bit.
pub const DAM_MAGIC: i32 = 1 << 5;
/// `DAM_POINT` bit.
pub const DAM_POINT: i32 = 1 << 6;
/// `DAM_FALL` bit.
pub const DAM_FALL: i32 = 1 << 7;

/// `DAM_INDEX_BARRIER` / `PROT_BARRIER` slot.
pub const DAM_INDEX_BARRIER: usize = 0;
/// `DAM_INDEX_BLUNT` / `PROT_BLUNT` slot.
pub const DAM_INDEX_BLUNT: usize = 1;
/// `DAM_INDEX_EDGE` / `PROT_EDGE` slot.
pub const DAM_INDEX_EDGE: usize = 2;
/// `DAM_INDEX_FIRE` / `PROT_FIRE` slot.
pub const DAM_INDEX_FIRE: usize = 3;
/// `DAM_INDEX_FLY` / `PROT_FLY` slot.
pub const DAM_INDEX_FLY: usize = 4;
/// `DAM_INDEX_MAGIC` / `PROT_MAGIC` slot.
pub const DAM_INDEX_MAGIC: usize = 5;
/// `DAM_INDEX_POINT` / `PROT_POINT` slot.
pub const DAM_INDEX_POINT: usize = 6;
/// `DAM_INDEX_FALL` / `PROT_FALL` slot.
pub const DAM_INDEX_FALL: usize = 7;
/// `DAM_INDEX_MAX` / `PROT_INDEX_MAX`, one past the last slot.
pub const DAM_INDEX_MAX: usize = DAMAGE_TYPES;

/// `PROT_BARRIER` protection slot.
pub const PROT_BARRIER: usize = DAM_INDEX_BARRIER;
/// `PROT_BLUNT` protection slot.
pub const PROT_BLUNT: usize = DAM_INDEX_BLUNT;
/// `PROT_EDGE` protection slot.
pub const PROT_EDGE: usize = DAM_INDEX_EDGE;
/// `PROT_FIRE` protection slot.
pub const PROT_FIRE: usize = DAM_INDEX_FIRE;
/// `PROT_FLY` protection slot.
pub const PROT_FLY: usize = DAM_INDEX_FLY;
/// `PROT_MAGIC` protection slot.
pub const PROT_MAGIC: usize = DAM_INDEX_MAGIC;
/// `PROT_POINT` protection slot.
pub const PROT_POINT: usize = DAM_INDEX_POINT;
/// `PROT_FALL` protection slot.
pub const PROT_FALL: usize = DAM_INDEX_FALL;
