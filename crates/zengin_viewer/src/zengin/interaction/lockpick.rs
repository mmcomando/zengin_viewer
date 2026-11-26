//! Opens a chest once its lockpick sequence is completed.

use bevy::log::warn;
use bevy::prelude::*;

use crate::gameplay::objects::NpcModel;
use crate::zengin::AnimationController;
use crate::zengin::PlayerCharacter;
use crate::zengin::gui::{LockpickOpened, LootSource, OpenLoot};
use crate::zengin::npc_actions::{
    MobUsePose, UseMobAction, enter_animation_name, mob_animation_name,
};
use crate::zengin::npc_routines::{AnimationFiles, NpcOverlay, queue_named_animation};
use crate::zengin::resources::CachedAssetHandles;
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::{GameChest, GameNpc, MobPose, NpcMobUse};
use dev_util::perf_debug;

#[allow(clippy::cognitive_complexity, clippy::type_complexity)]
pub(super) fn open_lockpicked_chest(
    mut commands: Commands,
    mut opened: MessageReader<LockpickOpened>,
    files: Option<Res<AnimationFiles>>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut chests: Query<(&mut GameChest, &mut MobPose)>,
    mut players: Query<
        (
            Entity,
            &GameNpc,
            Option<&mut NpcMobUse>,
            Option<&NpcModel>,
            Option<&NpcOverlay>,
            Option<&mut AnimationController>,
        ),
        With<PlayerCharacter>,
    >,
    mut requests: MessageWriter<OpenLoot>,
) {
    let _perf = perf_debug!();
    for event in opened.read() {
        let Ok((mut chest, mut pose)) = chests.get_mut(event.chest) else {
            warn!("Lockpick completed for missing chest({})", event.chest);
            continue;
        };
        chest.cracked = true;
        let Ok((entity, npc, mob_use, model, overlay, controller)) = players.single_mut() else {
            continue;
        };
        let player = npc.instance_id();

        if let Some(mut mob_use) = mob_use {
            if mob_use.mob != event.chest {
                warn!(
                    "Player({player}) is attached to mob({}) while chest({}) was picked",
                    mob_use.mob, event.chest
                );
                continue;
            }
            let target_state = 1;
            let scheme = mob_use.scheme.as_str();
            let tag = mob_use.position_tag.as_deref();
            if let (Some(model), Some(files), Some(mut controller)) =
                (model, files.as_deref(), controller)
            {
                let target_pose =
                    mob_animation_name(model, overlay, files, scheme, tag, target_state);
                if let Some(transition) = enter_animation_name(
                    model,
                    overlay,
                    files,
                    scheme,
                    tag,
                    mob_use.state,
                    target_state,
                ) {
                    queue_named_animation(
                        &mut controller,
                        model,
                        overlay,
                        files,
                        &mut handles,
                        &asset_server,
                        &transition,
                    );
                    commands
                        .entity(entity)
                        .insert(MobUsePose::new(transition, target_pose));
                } else {
                    queue_named_animation(
                        &mut controller,
                        model,
                        overlay,
                        files,
                        &mut handles,
                        &asset_server,
                        &target_pose,
                    );
                }
            }
            mob_use.state = target_state;
        } else if pose.state_num > 0 && !pose.scheme.is_empty() {
            commands.entity(entity).insert(UseMobAction::player(
                event.chest,
                StringCanon::new(&pose.scheme),
                1,
            ));
        }

        pose.state = 1;
        requests.write(OpenLoot {
            player,
            source: LootSource::Chest(event.chest),
        });
    }
}
