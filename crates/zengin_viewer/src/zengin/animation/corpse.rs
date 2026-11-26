//! Corpse animation: a killed npc plays its death transition and then loops
//! the final lying pose.

use bevy::prelude::*;

use super::{
    AnimationController, AnimationData, AutoAnimSequencer, DesiredAnimation,
    animation_name_from_path,
};
use crate::zengin::gameplay::objects::NpcModel;
use crate::zengin::{
    AnimationFiles, CachedAssetHandles, NpcCorpseMarker, animation_file_name, get_animation_path,
};
use dev_util::perf_debug;
use dev_util::warn_once;

const DEATH_TRANSITIONS: &[&str] = &["T_DEAD", "T_DEADB"];
const DEATH_POSES: &[&str] = &["S_DEAD", "S_DEADB"];

pub(crate) fn apply_corpse_animation(
    files: Res<AnimationFiles>,
    asset_server: Res<AssetServer>,
    mut handles: ResMut<CachedAssetHandles>,
    mut corpses: Query<
        (&NpcModel, &mut AnimationController, &mut AutoAnimSequencer),
        With<NpcCorpseMarker>,
    >,
) {
    let _perf = perf_debug!();
    for (model, mut controller, mut sequencer) in &mut corpses {
        let Some(scheme) = model.animation_scheme.as_deref() else {
            continue;
        };
        if sequencer.direction.is_some() {
            sequencer.direction = None;
        }
        if controller.root_motion() {
            controller.set_root_motion(false);
        }

        let current = handle_animation_name(controller.current());
        let queued = controller.next().and_then(handle_animation_name);
        if current.is_some_and(|name| DEATH_POSES.contains(&name)) {
            suspend_if_sequencer_drives(&mut sequencer);
            continue;
        }
        if current.is_some_and(|name| DEATH_TRANSITIONS.contains(&name))
            || queued.is_some_and(|name| {
                DEATH_TRANSITIONS.contains(&name) || DEATH_POSES.contains(&name)
            })
        {
            set_dead_desired(&mut sequencer);
            continue;
        }

        let next = DEATH_TRANSITIONS
            .iter()
            .chain(DEATH_POSES)
            .find(|animation| files.0.contains(&animation_file_name(scheme, animation)))
            .copied();
        let Some(next) = next else {
            warn_once!("Scheme({scheme}) has no death animation, corpse keeps its pose");
            suspend_if_sequencer_drives(&mut sequencer);
            continue;
        };
        let path = get_animation_path(scheme, next);
        controller.queue_next(handles.get_animation_handle(&asset_server, &path), true);
        if DEATH_POSES.contains(&next) {
            suspend_if_sequencer_drives(&mut sequencer);
        } else {
            set_dead_desired(&mut sequencer);
        }
    }
}

fn handle_animation_name(handle: &Handle<AnimationData>) -> Option<&str> {
    let path = Handle::path(handle)?;
    animation_name_from_path(path.path())
}

fn set_dead_desired(sequencer: &mut AutoAnimSequencer) {
    if sequencer.desired == Some(DesiredAnimation::Dead) && !sequencer.force_change {
        return;
    }
    sequencer.desired = Some(DesiredAnimation::Dead);
    sequencer.force_change = false;
}

fn suspend_if_sequencer_drives(sequencer: &mut AutoAnimSequencer) {
    if sequencer.desired.is_some() || sequencer.force_change {
        sequencer.suspend();
    }
}
