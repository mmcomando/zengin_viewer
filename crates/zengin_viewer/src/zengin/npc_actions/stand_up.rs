use bevy::prelude::*;

use crate::gameplay::objects::NpcModel;
use crate::graphics::{AnimationController, AutoAnimSequencer};
use crate::zengin::npc_actions::action::ExecutingActionMarker;
use crate::zengin::npc_actions::use_mob::release_mob_use;
use crate::zengin::npc_routines::{AnimationFiles, NpcOverlay, queue_stand_animation};
use crate::zengin::resources::CachedAssetHandles;
use crate::zengin::{GameMob, NpcMobUse};

/// Stops the npc and stands it up idle.
#[derive(Component)]
#[require(ExecutingActionMarker)]
pub(crate) struct StandUpAction;

#[allow(clippy::too_many_arguments)]
pub(crate) fn start_stand_up_action(
    mut commands: Commands,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut mobs: Query<&mut GameMob>,
    mut npcs: Query<
        (
            Entity,
            Option<&NpcMobUse>,
            Option<&mut AutoAnimSequencer>,
            Option<&mut AnimationController>,
            &NpcModel,
            Option<&NpcOverlay>,
        ),
        Added<StandUpAction>,
    >,
) {
    for (entity, npc_mob_use, mut sequencer, mut controller, model, overlay) in &mut npcs {
        if let Some(mob_use) = npc_mob_use
            && let Ok(mut mob) = mobs.get_mut(mob_use.mob)
        {
            mob.state = 0;
        }
        let has_mob_use = npc_mob_use.is_some();
        release_mob_use(
            &mut commands,
            entity,
            npc_mob_use,
            controller.as_deref_mut(),
            sequencer.as_mut().map(|sequencer| &mut **sequencer),
            model,
            overlay,
            &files,
            &mut handles,
            &asset_server,
        );
        if !has_mob_use && let Some(mut controller) = controller {
            controller.set_root_motion(false);
            queue_stand_animation(
                &mut controller,
                sequencer.as_mut().map(|sequencer| &mut **sequencer),
                model,
                overlay,
                &files,
                &mut handles,
                &asset_server,
            );
        }
        commands.entity(entity).remove::<ExecutingActionMarker>();
    }
}
