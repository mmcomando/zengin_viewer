//! Opening chests and corpses: the selected loot source requests the loot UI on
//! `Interact`.

use bevy::log::info;
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use super::selection::{Selection, SelectionKind};
use crate::gui::{LootSource, OpenLockpick, OpenLoot};
use crate::input::{GameplayAction, GameplayInputMarker};
use crate::zengin::PlayerCharacter;
use crate::zengin::npc_actions::UseMobAction;
use crate::zengin::npc_routines::NpcWeaponMode;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::manager::{ScriptManager, TALENT_PICKLOCK};
use crate::zengin::script::state::WeaponMode;
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::{GameChest, GameNpc, InstanceId, MobBase, MobPose, NpcCorpseMarker};
use dev_util::perf_debug;
use dev_util::test_log;

/// What the player may do with a chest lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChestAccess {
    Open,
    Key,
    Lockpick,
    MissingKey,
    MissingLockpick,
    MissingKeyOrLockpick,
}

fn chest_access(lock: ChestLock) -> ChestAccess {
    if !lock.locked || lock.cracked {
        return ChestAccess::Open;
    }
    if lock.has_key {
        return ChestAccess::Key;
    }
    if lock.pick_present && lock.can_pick {
        return ChestAccess::Lockpick;
    }
    if lock.key_present && lock.pick_present {
        return ChestAccess::MissingKeyOrLockpick;
    }
    if lock.key_present {
        return ChestAccess::MissingKey;
    }
    if lock.pick_present {
        return ChestAccess::MissingLockpick;
    }
    // A locked flag without a key or pick string gates nothing, matching the
    // engine.
    ChestAccess::Open
}

#[allow(clippy::struct_excessive_bools)]
struct ChestLock {
    locked: bool,
    cracked: bool,
    key_present: bool,
    pick_present: bool,
    has_key: bool,
    can_pick: bool,
}

fn resolve_access(
    script: &mut ScriptManager,
    player: InstanceId,
    chest: &GameChest,
) -> ChestAccess {
    let key_present = !chest.key.trim().is_empty();
    let pick_present = !chest.pick_string.trim().is_empty();
    let has_key = key_present
        && script
            .symbol_instance(&chest.key.trim().to_lowercase())
            .is_some_and(|item| script.item_count(player, item) > 0);
    let can_pick = pick_present
        && script.talent_skill(player, TALENT_PICKLOCK) != 0
        && script.has_lockpick(player);
    let lock = ChestLock {
        locked: chest.locked,
        cracked: chest.cracked,
        key_present,
        pick_present,
        has_key,
        can_pick,
    };
    let access = chest_access(lock);
    if let Some(function) = refusal_function(access) {
        script.call_player_refusal(player, function);
    }
    access
}

fn refusal_function(access: ChestAccess) -> Option<&'static str> {
    match access {
        ChestAccess::MissingKey => Some("player_mob_missing_key"),
        ChestAccess::MissingLockpick => Some("player_mob_missing_lockpick"),
        ChestAccess::MissingKeyOrLockpick => Some("player_mob_missing_key_or_lockpick"),
        ChestAccess::Open | ChestAccess::Key | ChestAccess::Lockpick => None,
    }
}

#[allow(clippy::cognitive_complexity, clippy::too_many_arguments)]
pub(super) fn open_selected_chest(
    mut commands: Commands,
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    selection: Res<Selection>,
    player_npc: Query<(Entity, &GameNpc), With<PlayerCharacter>>,
    player_weapon: Query<&NpcWeaponMode, With<PlayerCharacter>>,
    mut chests: Query<(&mut GameChest, &MobBase, &MobPose)>,
    gothic_script: Res<ScriptResource>,
    mut requests: MessageWriter<OpenLoot>,
    mut lockpick_requests: MessageWriter<OpenLockpick>,
) {
    let _perf = perf_debug!();
    if !action_state.just_pressed(&GameplayAction::Interact) {
        return;
    }
    let weapon_mode = player_weapon
        .single()
        .map_or(WeaponMode::Fists, |mode| mode.0);
    if weapon_mode != WeaponMode::Fists {
        return;
    }
    if selection.kind != Some(SelectionKind::Chest) {
        return;
    }
    let Some(target) = selection.target else {
        return;
    };
    let Ok((mut chest, base, pose)) = chests.get_mut(target) else {
        return;
    };
    let Ok((player_entity, player_npc)) = player_npc.single() else {
        return;
    };
    let player = player_npc.instance_id();

    let access = resolve_access(&mut gothic_script.lock(), player, &chest);

    match access {
        ChestAccess::MissingKey
        | ChestAccess::MissingLockpick
        | ChestAccess::MissingKeyOrLockpick => {
            test_log!(
                "Player({player}) cannot open locked chest({target}) vob({})",
                base.vob_name
            );
            return;
        }
        ChestAccess::Lockpick => {
            info!(
                "Player({player}) must pick the lock of chest({target}) vob({})",
                base.vob_name
            );
            lockpick_requests.write(OpenLockpick {
                player,
                chest: target,
            });
            if pose.state_num > 0 && !pose.scheme.is_empty() {
                commands
                    .entity(player_entity)
                    .insert(UseMobAction::player_switch(
                        target,
                        StringCanon::new(&pose.scheme),
                        0,
                    ));
            }
            return;
        }
        ChestAccess::Open | ChestAccess::Key => {}
    }
    if access == ChestAccess::Key {
        chest.cracked = true;
        test_log!(
            "Player({player}) unlocked chest({target}) vob({}) with key({})",
            base.vob_name,
            chest.key
        );
    }

    if pose.state_num > 0 && !pose.scheme.is_empty() {
        commands.entity(player_entity).insert(UseMobAction::player(
            target,
            StringCanon::new(&pose.scheme),
            1,
        ));
    }

    requests.write(OpenLoot {
        player,
        source: LootSource::Chest(target),
    });
}

/// A corpse has no pose to attach to, so the loot window opens without an npc
/// use action.
pub(super) fn open_selected_corpse(
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    selection: Res<Selection>,
    player_npc: Query<&GameNpc, With<PlayerCharacter>>,
    player_weapon: Query<&NpcWeaponMode, With<PlayerCharacter>>,
    corpses: Query<&GameNpc, With<NpcCorpseMarker>>,
    mut requests: MessageWriter<OpenLoot>,
) {
    let _perf = perf_debug!();
    if !action_state.just_pressed(&GameplayAction::Interact) {
        return;
    }
    let weapon_mode = player_weapon
        .single()
        .map_or(WeaponMode::Fists, |mode| mode.0);
    if weapon_mode != WeaponMode::Fists {
        return;
    }
    if selection.kind != Some(SelectionKind::Corpse) {
        return;
    }
    let Some(target) = selection.target else {
        return;
    };
    let Ok(_corpse) = corpses.get(target) else {
        return;
    };
    let Ok(player_npc) = player_npc.single() else {
        return;
    };

    requests.write(OpenLoot {
        player: player_npc.instance_id(),
        source: LootSource::Corpse(target),
    });
}

#[cfg(test)]
mod tests {
    use super::{ChestAccess, ChestLock, chest_access};

    fn access(
        locked: bool,
        cracked: bool,
        key: bool,
        pick: bool,
        has_key: bool,
        can_pick: bool,
    ) -> ChestAccess {
        chest_access(ChestLock {
            locked,
            cracked,
            key_present: key,
            pick_present: pick,
            has_key,
            can_pick,
        })
    }

    #[test]
    fn unlocked_and_cracked_chests_open() {
        assert_eq!(
            access(false, false, true, true, false, false),
            ChestAccess::Open
        );
        assert_eq!(
            access(true, true, true, true, false, false),
            ChestAccess::Open
        );
    }

    #[test]
    fn a_held_key_wins_over_lockpicking() {
        assert_eq!(
            access(true, false, true, true, true, true),
            ChestAccess::Key
        );
    }

    #[test]
    fn lockpicking_needs_talent_and_a_pick() {
        assert_eq!(
            access(true, false, false, true, false, true),
            ChestAccess::Lockpick
        );
        assert_eq!(
            access(true, false, false, true, false, false),
            ChestAccess::MissingLockpick
        );
    }

    #[test]
    fn missing_key_and_pick_requirements_are_reported() {
        assert_eq!(
            access(true, false, true, false, false, true),
            ChestAccess::MissingKey
        );
        assert_eq!(
            access(true, false, true, true, false, false),
            ChestAccess::MissingKeyOrLockpick
        );
    }

    #[test]
    fn a_lock_without_key_or_pick_opens() {
        assert_eq!(
            access(true, false, false, false, false, false),
            ChestAccess::Open
        );
    }

    #[test]
    fn key_or_pick_data_on_an_unlocked_chest_is_ignored() {
        assert_eq!(
            access(false, false, true, true, false, false),
            ChestAccess::Open
        );
    }
}
