//! Animation triggers read from the human model script, plus preloading of
//! the human animation assets.
//!
//! Compiled `.MAN` animations carry no events — the events (sound effects,
//! item place/remove tags, ...) live in the compiled model script
//! (`HUMANS.MDS` in the anims archive). [`AnimationTriggers`] exposes them
//! keyed by animation name so the playback can emit `AnimationTrigger` and
//! `AnimationSound` messages when an animation crosses a trigger or sound frame.

use std::collections::HashMap;
use std::path::Path;

use bevy::prelude::*;
use zen_kit_rs::model_scripts::ModelScript;
use zen_kit_rs::stream::Read;

use crate::zengin::{ZenginVfs, get_zengin_asset_bytes, to_world_units};

pub struct AnimationTriggersPlugin;

impl Plugin for AnimationTriggersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_human_animation_data);
    }
}

/// Compiled model script of the human visual scheme. In Gothic 2 compiled
/// `.MDS` scripts are stored with the `.MSB` extension (see `OpenGothic`'s
/// `Resources::implLoadAnimation`).
const HUMANS_MODEL_SCRIPT: &str = "_WORK/DATA/ANIMS/_COMPILED/HUMANS.MSB";

/// One animation event: `name` fires when the animation reaches `frame`.
#[derive(Debug, Clone)]
pub struct AnimationTriggerDef {
    pub frame: u32,
    pub name: String,
}

/// Animation triggers and sound effects by animation name (uppercased, e.g.
/// `S_BOWSHOOT`).
#[derive(Resource, Default)]
pub struct AnimationTriggers {
    animations: HashMap<String, Vec<AnimationTriggerDef>>,
    sounds: HashMap<String, Vec<AnimationSoundDef>>,
}

impl AnimationTriggers {
    pub fn triggers(&self, animation: &str) -> &[AnimationTriggerDef] {
        self.animations
            .get(&animation.to_uppercase())
            .map_or(&[], |triggers| triggers)
    }

    pub fn sounds(&self, animation: &str) -> &[AnimationSoundDef] {
        self.sounds
            .get(&animation.to_uppercase())
            .map_or(&[], |sounds| sounds)
    }
}

/// One sound effect of an animation: the sound definition `name` plays when the
/// animation reaches `frame`; `range` is the audible distance in world units.
#[derive(Debug, Clone)]
pub struct AnimationSoundDef {
    pub frame: u32,
    pub name: String,
    pub range: f32,
}

/// Animation name from a zengin asset path:
/// `zengin://.../_COMPILED/HUMANS-S_RUN.MAN` -> `S_RUN`.
/// Schemes use underscores only, so the first `-` separates the scheme from
/// the animation name.
pub fn animation_name_from_path(path: &Path) -> Option<&str> {
    let file = path.file_name()?.to_str()?;
    let stem = file.strip_suffix(".MAN")?;
    stem.split_once('-').map(|(_scheme, name)| name)
}

/// Triggers whose frame the animation crossed between `old_progress` and
/// `new_progress` (in frames). Frames are half-open intervals `[old, new)`, so
/// a trigger at frame `0` fires when the animation starts. A step that wrapped
/// around the loop covers everything from `old_progress` around to
/// `new_progress`, so a trigger may lie in either part.
pub fn crossed_triggers(
    triggers: &[AnimationTriggerDef],
    old_progress: f32,
    new_progress: f32,
    crossed: bool,
) -> impl Iterator<Item = &AnimationTriggerDef> + '_ {
    triggers.iter().filter(move |trigger| {
        let target = trigger.frame as f32;
        if crossed {
            target >= old_progress || target < new_progress
        } else {
            target >= old_progress && target < new_progress
        }
    })
}

/// Sound effects whose frame the animation crossed between `old_progress` and
/// `new_progress`, with the same frame intervals as [`crossed_triggers`].
pub fn crossed_sounds(
    sounds: &[AnimationSoundDef],
    old_progress: f32,
    new_progress: f32,
    crossed: bool,
) -> impl Iterator<Item = &AnimationSoundDef> + '_ {
    sounds.iter().filter(move |sound| {
        let target = sound.frame as f32;
        if crossed {
            target >= old_progress || target < new_progress
        } else {
            target >= old_progress && target < new_progress
        }
    })
}

/// Loads the model script triggers. Best effort: without the script no
/// triggers are registered.
fn load_human_animation_data(mut commands: Commands, vfs: Res<ZenginVfs>) {
    let (triggers, source) = load_triggers_or_fallback(&vfs);

    let animations = triggers.animations.len();
    let trigger_count: usize = triggers.animations.values().map(Vec::len).sum();
    let sound_count: usize = triggers.sounds.values().map(Vec::len).sum();
    info!(
        "Loaded animation triggers({animations}, {trigger_count}) and sounds({sound_count}) from {source}"
    );
    for (animation, animation_triggers) in &triggers.animations {
        if animation_triggers.is_empty() {
            continue;
        }
        debug!("Triggers({animation}): {animation_triggers:?}");
    }
    for (animation, animation_sounds) in &triggers.sounds {
        if animation_sounds.is_empty() {
            continue;
        }
        debug!("Sounds({animation}): {animation_sounds:?}");
    }

    commands.insert_resource(triggers);
}

/// Loads the model script triggers, falling back to an empty set when the
/// script is missing or unparsable.
fn load_triggers_or_fallback(vfs: &ZenginVfs) -> (AnimationTriggers, &'static str) {
    let bytes = {
        let vfs = vfs.0.lock().unwrap();
        get_zengin_asset_bytes(&vfs, HUMANS_MODEL_SCRIPT)
    };
    let Some(bytes) = bytes else {
        warn!(
            "Model script({HUMANS_MODEL_SCRIPT}) not found in the anims archive, animation triggers are not loaded"
        );
        return (AnimationTriggers::default(), "script missing");
    };
    match load_script_triggers(&bytes) {
        Some(triggers) => (triggers, "HUMANS.MDS"),
        None => (AnimationTriggers::default(), "script parse failed"),
    }
}

/// Parses the compiled model script into per-animation triggers and sound
/// effects. Event tags combine the event type and item (e.g.
/// `ITEM_INSERT_ITRW_ARROW`), sound effects use the sound definition name.
/// Frame lists expand to one trigger per frame.
fn load_script_triggers(bytes: &[u8]) -> Option<AnimationTriggers> {
    let read = Read::from_slice(bytes)?;
    let script = ModelScript::load(&read)?;
    let mut animations = HashMap::new();
    let mut sounds = HashMap::new();

    for animation in script.animations() {
        let mut triggers = Vec::new();
        let mut animation_sounds = Vec::new();
        // Model script sound frames are absolute animation frames, while the
        // playback progress runs from 0 within the played animation.
        let first_frame = animation.first_frame();

        for effect in animation.sound_effects() {
            if effect.frame() < 0 {
                continue;
            }
            animation_sounds.push(AnimationSoundDef {
                frame: (effect.frame() - first_frame).max(0) as u32,
                name: effect.name(),
                range: to_world_units(effect.range()),
            });
        }

        for tag in animation.event_tags() {
            let name = format!("{:?}_{}", tag.event_type(), tag.item());
            // Tags with a frame list carry frames relative to the animation
            // start; the single frame form uses absolute model script frames.
            let frames = tag.frames();
            if frames.is_empty() {
                triggers.push(AnimationTriggerDef {
                    frame: (tag.frame() - first_frame).max(0) as u32,
                    name,
                });
                continue;
            }
            for frame in frames {
                if frame >= 0 {
                    triggers.push(AnimationTriggerDef {
                        frame: frame as u32,
                        name: name.clone(),
                    });
                }
            }
        }

        triggers.sort_by_key(|trigger| trigger.frame);
        animation_sounds.sort_by_key(|sound| sound.frame);
        let name = animation.name().to_uppercase();
        animations.insert(name.clone(), triggers);
        sounds.insert(name, animation_sounds);
    }

    Some(AnimationTriggers { animations, sounds })
}
