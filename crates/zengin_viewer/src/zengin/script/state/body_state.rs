//! Body state bits of the daedalus `BS_*` constants and the mob scheme to body
//! state mapping.

use super::instances::NpcLiveState;

/// `MOB_SIT`, `MOB_LIE`, `MOB_CLIMB` and `MOB_NOTINTERRUPTABLE` scheme lists,
/// each a comma separated list of `AI_UseMob` schemes.
#[derive(Debug, Clone)]
pub(crate) struct MobSchemeLists {
    pub sit: String,
    pub lie: String,
    pub climb: String,
    pub not_interruptable: String,
}

impl Default for MobSchemeLists {
    fn default() -> Self {
        Self {
            sit: "BENCH,CHAIR,GROUND,THRONE".to_string(),
            lie: "BED,BEDHIGH,BEDLOW".to_string(),
            climb: "CLIMB,LADDER,RANKE".to_string(),
            not_interruptable: "DOOR,LEVER,TOUCHPLATE,TURNSWITCH,VWHEEL,CHESTBIG,CHESTSMALL,HERB,IDOL,PAN,SMOKE,INNOS"
                .to_string(),
        }
    }
}

/// `BS_STAND`: interruptable with free hands, the state of an idle npc.
pub(crate) const BS_STAND: i32 = BS_FLAG_INTERRUPTABLE | BS_FLAG_FREEHANDS;
const BS_DEAD: i32 = 23;
const BS_SIT: i32 = 0b1011 | BS_FLAG_FREEHANDS;
const BS_LIE: i32 = 12;
const BS_CLIMB: i32 = 0b1001 | BS_FLAG_INTERRUPTABLE;
const BS_MOBINTERACT: i32 = 15;
const BS_MOBINTERACT_INTERRUPT: i32 = 0b1_0000 | BS_FLAG_INTERRUPTABLE;

const BS_FLAG_INTERRUPTABLE: i32 = 1 << 15;
const BS_FLAG_FREEHANDS: i32 = 1 << 16;
const BS_MOD_HIDDEN: i32 = 1 << 7;
const BS_MOD_DRUNK: i32 = 1 << 8;
const BS_MOD_NUTS: i32 = 1 << 9;
const BS_MOD_BURNING: i32 = 1 << 10;
const BS_MOD_CONTROLLED: i32 = 1 << 11;

/// Body state an npc has while attached to a mob with `scheme`. Schemes missing
/// from every list produce `BS_MOBINTERACT_INTERRUPT`, the engine default.
pub(crate) fn body_state_for_scheme(scheme: &str, lists: &MobSchemeLists) -> i32 {
    let scheme = scheme.to_uppercase();
    if scheme_in_list(&scheme, &lists.sit) {
        return BS_SIT;
    }
    if scheme_in_list(&scheme, &lists.lie) {
        return BS_LIE;
    }
    if scheme_in_list(&scheme, &lists.climb) {
        return BS_CLIMB;
    }
    if scheme_in_list(&scheme, &lists.not_interruptable) {
        return BS_MOBINTERACT;
    }
    BS_MOBINTERACT_INTERRUPT
}

/// Body state reported to the scripts: death overrides, the mob interaction
/// override wins over standing.
pub(crate) fn current_body_state(live: &NpcLiveState) -> i32 {
    if live.dead {
        return BS_DEAD;
    }
    live.body_state.unwrap_or(BS_STAND)
}

/// `Npc_HasBodyFlag` semantics: the flag bits of the body state have to match
/// `flag` exactly.
pub(crate) fn has_body_flag(body_state: i32, flag: i32) -> bool {
    let mask = BS_FLAG_INTERRUPTABLE
        | BS_MOD_HIDDEN
        | BS_MOD_DRUNK
        | BS_MOD_NUTS
        | BS_MOD_BURNING
        | BS_MOD_CONTROLLED;
    flag == body_state & mask
}

fn scheme_in_list(scheme: &str, list: &str) -> bool {
    list.split(',').any(|entry| entry.trim() == scheme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mob_scheme_lists_map_to_their_body_state() {
        let lists = MobSchemeLists::default();

        assert_eq!(body_state_for_scheme("BENCH", &lists), BS_SIT);
        assert_eq!(body_state_for_scheme("bedhigh", &lists), BS_LIE);
        assert_eq!(body_state_for_scheme("LADDER", &lists), BS_CLIMB);
        assert_eq!(body_state_for_scheme("HERB", &lists), BS_MOBINTERACT);
        assert_eq!(
            body_state_for_scheme("BSANVIL", &lists),
            BS_MOBINTERACT_INTERRUPT
        );
    }

    #[test]
    fn body_flag_matches_the_interruptable_bit() {
        let lists = MobSchemeLists::default();

        assert!(has_body_flag(
            body_state_for_scheme("BSANVIL", &lists),
            BS_FLAG_INTERRUPTABLE
        ));
        assert!(!has_body_flag(
            body_state_for_scheme("BENCH", &lists),
            BS_FLAG_INTERRUPTABLE
        ));
    }
}
