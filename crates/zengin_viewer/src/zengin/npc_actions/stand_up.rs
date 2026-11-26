use bevy::prelude::*;

use crate::zengin::npc_actions::action::ExecutingActionMarker;
use crate::zengin::npc_actions::use_mob::release_mob_use;
use crate::zengin::npc_routines::queue_stand_animation;
use crate::zengin::{AnimationController, AutoAnimSequencer};
use crate::zengin::{MobPose, NpcMobUse};
use dev_util::perf_debug;

/// Stops the npc and stands it up idle.
#[derive(Component)]
#[require(ExecutingActionMarker)]
pub(crate) struct StandUpAction;

pub(crate) fn start_stand_up_action(
    mut commands: Commands,
    mut mobs: Query<&mut MobPose>,
    mut npcs: Query<
        (
            Entity,
            Option<&NpcMobUse>,
            &mut AutoAnimSequencer,
            &mut AnimationController,
        ),
        Added<StandUpAction>,
    >,
) {
    let _perf = perf_debug!();
    for (entity, npc_mob_use, mut sequencer, mut controller) in &mut npcs {
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
            &mut controller,
            &mut sequencer,
        );
        if !has_mob_use {
            controller.set_root_motion(false);
            queue_stand_animation(&mut sequencer);
        }
        commands.entity(entity).remove::<ExecutingActionMarker>();
    }
}
