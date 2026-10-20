//! Animation triggers read from the human model script, plus preloading of
//! the human animation assets.
//!
//! Compiled `.MAN` animations carry no events — the events (sound effects,
//! item place/remove tags, ...) live in the compiled model script
//! (`HUMANS.MDS` in the anims archive). [`AnimationTriggers`] exposes them
//! keyed by animation name so the playback can emit `AnimationTrigger`
//! messages when an animation crosses a trigger frame. Entries missing from
//! the script fall back to [`FALLBACK_TRIGGERS`].

use std::collections::HashMap;
use std::path::Path;

use bevy::prelude::*;
use zen_kit_rs::model_scripts::ModelScript;
use zen_kit_rs::stream::Read;

use crate::zengin::loaders::vdf_reader::{ZenginVfs, get_zengin_asset_bytes};
use crate::zengin::zengin_resources::MaterialHandles;

/// Compiled model script of the human visual scheme. In Gothic 2 compiled
/// `.MDS` scripts are stored with the `.MSB` extension (see OpenGothic's
/// `Resources::implLoadAnimation`).
const HUMANS_MODEL_SCRIPT: &str = "_WORK/DATA/ANIMS/_COMPILED/HUMANS.MSB";

/// Hand-written triggers for animations the model script does not describe.
/// Filled from the script dump as sequencer states need them.
const FALLBACK_TRIGGERS: &[(&str, &[(u32, &str)])] = &[];

/// One animation event: `name` fires when the animation reaches `frame`.
/// Read by the sequencer phase; until then only built and logged.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AnimationTriggerDef {
    pub frame: u32,
    pub name: String,
}

/// Animation triggers by animation name (uppercased, e.g. `S_BOWSHOOT`).
#[allow(dead_code)]
#[derive(Resource, Default)]
pub struct AnimationTriggers {
    animations: HashMap<String, Vec<AnimationTriggerDef>>,
}

impl AnimationTriggers {
    #[allow(dead_code)]
    pub fn triggers(&self, animation: &str) -> &[AnimationTriggerDef] {
        self.animations
            .get(&animation.to_uppercase())
            .map_or(&[], |triggers| triggers)
    }
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
/// `new_progress` (in frames). A step that wrapped through full loops covers
/// everything from `old_progress` around to `new_progress`, so a trigger may
/// lie in either part; triggers inside fully skipped loops fire once.
pub fn crossed_triggers<'a>(
    triggers: &'a [AnimationTriggerDef],
    old_progress: f32,
    new_progress: f32,
    loops: u32,
) -> impl Iterator<Item = &'a AnimationTriggerDef> + 'a {
    triggers.iter().filter(move |trigger| {
        let target = trigger.frame as f32;
        if loops == 0 {
            target > old_progress && target <= new_progress
        } else {
            target > old_progress || target <= new_progress
        }
    })
}

pub struct HumanAnimationsPlugin;

impl Plugin for HumanAnimationsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_human_animation_data);
    }
}

/// Loads the model script triggers and preloads the sequencer animation
/// handles. Both are best effort: without the script the fallback table is
/// used, without an animation file its handle is simply absent.
fn load_human_animation_data(
    mut commands: Commands,
    vfs: Res<ZenginVfs>,
    asset_server: Res<AssetServer>,
    mut handles: ResMut<MaterialHandles>,
) {
    let bytes = {
        let vfs = vfs.0.lock().unwrap();
        get_zengin_asset_bytes(&vfs, HUMANS_MODEL_SCRIPT)
    };
    let (mut triggers, source) = match bytes {
        Some(bytes) => match load_script_triggers(&bytes) {
            Some(triggers) => (triggers, "HUMANS.MDS"),
            None => (HashMap::new(), "fallback"),
        },
        None => {
            warn!(
                "Model script({HUMANS_MODEL_SCRIPT}) not found in the anims archive, animation triggers fall back to the builtin table"
            );
            (HashMap::new(), "fallback")
        }
    };

    let fallbacks = FALLBACK_TRIGGERS.len();
    for (animation, fallback) in FALLBACK_TRIGGERS {
        let existing = triggers.entry(animation.to_uppercase()).or_default();
        for (frame, name) in fallback.iter() {
            existing.push(AnimationTriggerDef {
                frame: *frame,
                name: name.to_string(),
            });
        }
    }

    let animations: usize = triggers.len();
    let trigger_count: usize = triggers.values().map(Vec::len).sum();
    info!(
        "Loaded animation triggers({animations}, {trigger_count}) from {source} (+{fallbacks} fallback entries)"
    );
    for (animation, animation_triggers) in &triggers {
        if animation_triggers.is_empty() {
            continue;
        }
        debug!("Triggers({animation}): {animation_triggers:?}");
    }

    commands.insert_resource(AnimationTriggers {
        animations: triggers,
    });
}

/// Parses the compiled model script into per-animation triggers. Sound
/// effects use their sound name as the trigger name, event tags combine the
/// event type and item (e.g. `ITEM_INSERT_ITRW_ARROW`). Frame lists expand to
/// one trigger per frame.
fn load_script_triggers(bytes: &[u8]) -> Option<HashMap<String, Vec<AnimationTriggerDef>>> {
    let read = Read::from_slice(bytes)?;
    let script = ModelScript::load(&read)?;
    let mut animations = HashMap::new();

    for animation in script.animations() {
        let mut triggers = Vec::new();

        for effect in animation.sound_effects() {
            if effect.frame() < 0 {
                continue;
            }
            triggers.push(AnimationTriggerDef {
                frame: effect.frame() as u32,
                name: effect.name(),
            });
        }

        for tag in animation.event_tags() {
            let name = format!("{:?}_{}", tag.event_type(), tag.item());
            let frames = if tag.frame() >= 0 {
                vec![tag.frame()]
            } else {
                tag.frames()
            };
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
        animations.insert(animation.name().to_uppercase(), triggers);
    }

    Some(animations)
}
