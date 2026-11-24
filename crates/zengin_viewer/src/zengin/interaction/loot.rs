//! Opening chests and corpses: the selected loot source requests the loot UI on
//! `Interact`.

use bevy::log::{info, warn};
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use super::selection::{Selection, SelectionKind};
use crate::gameplay::objects::PlayerCharacter;
use crate::gui::{GameLog, LootSource, OpenLoot};
use crate::input::{GameplayAction, GameplayInputMarker};
use crate::perf_debug;
use crate::zengin::npc_actions::UseMobAction;
use crate::zengin::npc_routines::NpcWeaponMode;
use crate::zengin::script::state::WeaponMode;
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::{GameChest, GameNpc, MobBase, MobPose, NpcCorpseMarker};

pub(super) fn open_selected_chest(
    mut commands: Commands,
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    selection: Res<Selection>,
    player_npc: Query<(Entity, &GameNpc), With<PlayerCharacter>>,
    player_weapon: Query<&NpcWeaponMode, With<PlayerCharacter>>,
    chests: Query<(&GameChest, &MobBase, &MobPose)>,
    mut game_log: ResMut<GameLog>,
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
    if selection.kind != Some(SelectionKind::Chest) {
        return;
    }
    let Some(target) = selection.target else {
        return;
    };
    let Ok((chest, base, pose)) = chests.get(target) else {
        return;
    };
    let Ok((player_entity, player_npc)) = player_npc.single() else {
        return;
    };

    if chest.locked {
        warn!(
            "Chest({target}) vob({}) is locked, lock ignored",
            base.vob_name
        );
        game_log.add("Chest is locked (lock ignored)".to_string());
    }
    if pose.state_num > 0 && !pose.scheme.is_empty() {
        commands.entity(player_entity).insert(UseMobAction::player(
            target,
            StringCanon::new(&pose.scheme),
            1,
        ));
    }

    info!(
        "Player({}) opened chest({target}) vob({})",
        player_npc.instance_id(),
        base.vob_name
    );
    requests.write(OpenLoot {
        player: player_npc.instance_id(),
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
    let Ok(corpse) = corpses.get(target) else {
        return;
    };
    let Ok(player_npc) = player_npc.single() else {
        return;
    };

    info!(
        "Player({}) opened corpse({target}) npc({})",
        player_npc.instance_id(),
        corpse.instance_id()
    );
    requests.write(OpenLoot {
        player: player_npc.instance_id(),
        source: LootSource::Corpse(target),
    });
}
