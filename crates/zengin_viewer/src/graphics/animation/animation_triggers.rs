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
use dev_util::perf_debug;

pub struct AnimationTriggersPlugin;

impl Plugin for AnimationTriggersPlugin {
    fn build(&self, app: &mut App) {
        // Present from the start so systems can require it; the startup loader
        // replaces it with the parsed model script data.
        app.init_resource::<AnimationTriggers>();
        app.add_systems(Startup, load_human_animation_data);
    }
}

/// Compiled model script of the human visual scheme. In Gothic 2 compiled
/// `.MDS` scripts are stored with the `.MSB` extension.
const HUMANS_MODEL_SCRIPT: &str = "_WORK/DATA/ANIMS/_COMPILED/HUMANS.MSB";

/// Scheme of [`HUMANS_MODEL_SCRIPT`], used for the bare name fallback.
const HUMANS_SCHEME: &str = "HUMANS";

/// Weapon talent overlays carrying their own attack combo timings.
const SKILL_MODEL_SCRIPTS: &[(&str, &str)] = &[
    (
        "HUMANS_1HST1",
        "_WORK/DATA/ANIMS/_COMPILED/HUMANS_1HST1.MSB",
    ),
    (
        "HUMANS_1HST2",
        "_WORK/DATA/ANIMS/_COMPILED/HUMANS_1HST2.MSB",
    ),
    (
        "HUMANS_2HST1",
        "_WORK/DATA/ANIMS/_COMPILED/HUMANS_2HST1.MSB",
    ),
    (
        "HUMANS_2HST2",
        "_WORK/DATA/ANIMS/_COMPILED/HUMANS_2HST2.MSB",
    ),
];

/// One animation event: `name` fires when the animation reaches `frame`.
#[derive(Debug, Clone)]
pub struct AnimationTriggerDef {
    pub frame: u32,
    pub name: String,
}

/// Combo timing of one attack animation: the end frame of every hit segment and
/// the frame ranges in which a press chains to the next segment.
#[derive(Debug, Clone, Default)]
pub struct AttackTimeline {
    pub hit_end: Vec<u32>,
    pub windows: Vec<(u32, u32)>,
}

impl AttackTimeline {
    /// Frame range in which a press chains the segment `combo_len`.
    pub fn window(&self, combo_len: usize) -> Option<(u32, u32)> {
        self.windows.get(combo_len).copied()
    }

    /// Whether `combo_len` has a following segment to chain into.
    pub fn can_chain(&self, combo_len: usize) -> bool {
        combo_len + 1 < self.hit_end.len()
    }
}

/// Animation triggers and sound effects by animation name (uppercased, e.g.
/// `S_BOWSHOOT`). Scheme specific entries take precedence over the `HUMANS`
/// fallback, the weapon overlays redefine the same animation names.
#[derive(Resource, Default)]
pub struct AnimationTriggers {
    animations: HashMap<String, Vec<AnimationTriggerDef>>,
    scheme_animations: HashMap<String, HashMap<String, Vec<AnimationTriggerDef>>>,
    sounds: HashMap<String, Vec<AnimationSoundDef>>,
    scheme_sounds: HashMap<String, HashMap<String, Vec<AnimationSoundDef>>>,
    items: HashMap<String, Vec<AnimationItemEventDef>>,
    timelines: HashMap<String, HashMap<String, AttackTimeline>>,
}

impl AnimationTriggers {
    /// Trigger names are the uppercase archive names (`HUMANS_1HST1-S_1HATTACK`
    /// splits into `HUMANS_1HST1` and `S_1HATTACK`).
    pub fn triggers(&self, scheme: &str, animation: &str) -> &[AnimationTriggerDef] {
        self.scheme_animations
            .get(scheme)
            .and_then(|animations| animations.get(animation))
            .or_else(|| self.animations.get(animation))
            .map_or(&[], |triggers| triggers)
    }

    pub fn sounds(&self, scheme: &str, animation: &str) -> &[AnimationSoundDef] {
        self.scheme_sounds
            .get(scheme)
            .and_then(|animations| animations.get(animation))
            .or_else(|| self.sounds.get(animation))
            .map_or(&[], |sounds| sounds)
    }

    /// Combo timing of an attack animation, with the `HUMANS` fallback.
    pub fn timeline(&self, scheme: &str, animation: &str) -> Option<&AttackTimeline> {
        self.timelines
            .get(scheme)
            .and_then(|animations| animations.get(animation))
            .or_else(|| self.timelines.get(HUMANS_SCHEME)?.get(animation))
    }

    /// Item slot events (`ITEM_CREATE`, `ITEM_INSERT`, ...) of an animation.
    pub fn item_events(&self, animation: &str) -> &[AnimationItemEventDef] {
        self.items
            .get(&animation.to_uppercase())
            .map_or(&[], |events| events)
    }

    fn merge_scheme(&mut self, scheme: &str, data: SchemeTriggers) {
        let scheme_animations = self
            .scheme_animations
            .entry(scheme.to_string())
            .or_default();
        for (animation, triggers) in data.animations {
            if scheme == HUMANS_SCHEME {
                self.animations.insert(animation.clone(), triggers.clone());
            }
            scheme_animations.insert(animation, triggers);
        }
        let scheme_sounds = self.scheme_sounds.entry(scheme.to_string()).or_default();
        for (animation, sounds) in data.sounds {
            if scheme == HUMANS_SCHEME {
                self.sounds.insert(animation.clone(), sounds.clone());
            }
            scheme_sounds.insert(animation, sounds);
        }
        if scheme == HUMANS_SCHEME {
            self.items = data.items;
        }
        let scheme_timelines = self.timelines.entry(scheme.to_string()).or_default();
        for (animation, timeline) in data.timelines {
            scheme_timelines.insert(animation, timeline);
        }
    }
}

/// One item slot event: the event `kind` runs at `frame`, placing `item` into
/// the skeleton `slot`. `kind` is the event type name (e.g. `ITEM_INSERT`).
#[derive(Debug, Clone)]
pub struct AnimationItemEventDef {
    pub frame: u32,
    pub kind: String,
    pub item: String,
    pub slot: String,
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
    animation_ident_from_path(path).map(|(_scheme, animation)| animation)
}

/// Scheme and animation name from a zengin asset path:
/// `HUMANS_1HST1-S_1HATTACK.MAN` -> `("HUMANS_1HST1", "S_1HATTACK")`.
pub fn animation_ident_from_path(path: &Path) -> Option<(&str, &str)> {
    let file = path.file_name()?.to_str()?;
    let stem = file.strip_suffix(".MAN")?;
    stem.split_once('-')
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
#[allow(clippy::cognitive_complexity)]
fn load_human_animation_data(mut commands: Commands, vfs: Res<ZenginVfs>) {
    let _perf = perf_debug!();
    let (triggers, source) = load_triggers_or_fallback(&vfs);

    let animations: usize = triggers.scheme_animations.values().map(HashMap::len).sum();
    let trigger_count: usize = triggers
        .scheme_animations
        .values()
        .flat_map(|animations| animations.values())
        .map(Vec::len)
        .sum();
    let sound_count: usize = triggers
        .scheme_sounds
        .values()
        .flat_map(|animations| animations.values())
        .map(Vec::len)
        .sum();
    info!(
        "Loaded animation triggers({animations}, {trigger_count}) and sounds({sound_count}) from {source}"
    );
    for (scheme, animations) in &triggers.scheme_animations {
        for (animation, animation_triggers) in animations {
            if animation_triggers.is_empty() {
                continue;
            }
            debug!("Triggers({scheme}-{animation}): {animation_triggers:?}");
        }
    }
    for (scheme, sounds) in &triggers.scheme_sounds {
        for (animation, animation_sounds) in sounds {
            if animation_sounds.is_empty() {
                continue;
            }
            debug!("Sounds({scheme}-{animation}): {animation_sounds:?}");
        }
    }

    commands.insert_resource(triggers);
}

/// Loads the model script triggers, falling back to an empty set when the
/// script is missing or unparsable.
fn load_triggers_or_fallback(vfs: &ZenginVfs) -> (AnimationTriggers, &'static str) {
    let mut triggers = AnimationTriggers::default();
    let mut loaded = false;
    let vfs = vfs.0.lock().unwrap();
    for (scheme, path) in std::iter::once((HUMANS_SCHEME, HUMANS_MODEL_SCRIPT))
        .chain(SKILL_MODEL_SCRIPTS.iter().copied())
    {
        let Some(bytes) = get_zengin_asset_bytes(&vfs, path) else {
            warn!(
                "Model script({path}) not found in the anims archive, its triggers are not loaded"
            );
            continue;
        };
        let Some(scheme_triggers) = load_script_triggers(&bytes) else {
            warn!("Model script({path}) failed to parse, its triggers are not loaded");
            continue;
        };
        triggers.merge_scheme(scheme, scheme_triggers);
        loaded = true;
    }
    if !loaded {
        return (AnimationTriggers::default(), "script missing");
    }
    (triggers, "model scripts")
}

/// Per-animation data of one compiled model script.
#[derive(Default)]
struct SchemeTriggers {
    animations: HashMap<String, Vec<AnimationTriggerDef>>,
    sounds: HashMap<String, Vec<AnimationSoundDef>>,
    items: HashMap<String, Vec<AnimationItemEventDef>>,
    timelines: HashMap<String, AttackTimeline>,
}

/// Parses the compiled model script into per-animation triggers, sound effects
/// and attack combo timings. Event tags combine the event type and item (e.g.
/// `ITEM_INSERT_ITRW_ARROW`), sound effects use the sound definition name.
/// Frame lists expand to one trigger per frame.
fn load_script_triggers(bytes: &[u8]) -> Option<SchemeTriggers> {
    let read = Read::from_slice(bytes)?;
    let script = ModelScript::load(&read)?;
    let mut data = SchemeTriggers::default();

    for animation in script.animations() {
        let mut triggers = Vec::new();
        let mut animation_sounds = Vec::new();
        let mut animation_items = Vec::new();
        let mut hit_end = Vec::new();
        let mut window_frames = Vec::new();
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
            let kind = format!("{:?}", tag.event_type());
            let item = tag.item();
            let slot = tag.slot();
            if slot.is_empty() {
                if kind.starts_with("ITEM_") {
                    debug!(
                        "Animation({}) item event({kind}, item={item}) has no slot, skipped",
                        animation.name()
                    );
                }
            } else {
                let frame = (tag.frame() - first_frame).max(0) as u32;
                animation_items.push(AnimationItemEventDef {
                    frame,
                    kind: kind.clone(),
                    item,
                    slot,
                });
            }
            // Tags with a frame list carry frames relative to the animation
            // start; the single frame form uses absolute model script frames.
            let frames = tag.frames();
            let positive_frames = || frames.iter().copied().filter(|frame| *frame >= 0);
            if kind == "HIT_END" {
                hit_end.extend(positive_frames().map(|frame| frame as u32));
            } else if kind == "COMBO_WINDOW" {
                window_frames.extend(positive_frames().map(|frame| frame as u32));
            }
            let name = format!("{kind}_{}", tag.item());
            if frames.is_empty() {
                triggers.push(AnimationTriggerDef {
                    frame: (tag.frame() - first_frame).max(0) as u32,
                    name,
                });
                continue;
            }
            for frame in positive_frames() {
                triggers.push(AnimationTriggerDef {
                    frame: frame as u32,
                    name: name.clone(),
                });
            }
        }

        triggers.sort_by_key(|trigger| trigger.frame);
        animation_sounds.sort_by_key(|sound| sound.frame);
        animation_items.sort_by_key(|event| event.frame);
        let name = animation.name().to_uppercase();
        if !hit_end.is_empty() {
            data.timelines.insert(
                name.clone(),
                AttackTimeline {
                    hit_end,
                    windows: window_frames
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|pair| (pair[0], pair[1]))
                        .collect(),
                },
            );
        }
        data.animations.insert(name.clone(), triggers);
        data.sounds.insert(name.clone(), animation_sounds);
        data.items.insert(name, animation_items);
    }

    Some(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timeline(hit_end: &[u32], windows: &[(u32, u32)]) -> AttackTimeline {
        AttackTimeline {
            hit_end: hit_end.to_vec(),
            windows: windows.to_vec(),
        }
    }

    #[test]
    fn untrained_combo_has_two_segments_and_one_window() {
        let timeline = timeline(&[21, 39], &[(12, 26)]);

        assert!(timeline.can_chain(0));
        assert!(!timeline.can_chain(1));
        assert_eq!(timeline.window(0), Some((12, 26)));
        assert_eq!(timeline.window(1), None);
    }

    #[test]
    fn skill_overlay_combos_have_four_segments() {
        let timeline = timeline(&[33, 68, 103, 119], &[(12, 36), (47, 71), (82, 106)]);

        assert!(timeline.can_chain(0));
        assert!(timeline.can_chain(1));
        assert!(timeline.can_chain(2));
        assert!(!timeline.can_chain(3));
    }

    #[test]
    fn extra_window_after_the_last_segment_is_ignored() {
        let timeline = timeline(
            &[28, 60, 92, 113],
            &[(10, 33), (42, 65), (78, 97), (110, 113)],
        );

        assert!(!timeline.can_chain(3));
        assert_eq!(timeline.window(3), Some((110, 113)));
    }

    #[test]
    fn animation_ident_splits_scheme_from_animation() {
        let path = Path::new("HUMANS_1HST1-S_1HATTACK.MAN");

        assert_eq!(
            animation_ident_from_path(path),
            Some(("HUMANS_1HST1", "S_1HATTACK"))
        );
        assert_eq!(animation_name_from_path(path), Some("S_1HATTACK"));
    }
}
