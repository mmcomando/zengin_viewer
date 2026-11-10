//! On-disk save game format and file IO.
//!
//! The save is a single RON document. The types here are the on-disk contract:
//! they deliberately use raw script ids and plain data instead of the runtime
//! handles, so script/ECS refactors do not silently change the format.

use std::path::PathBuf;

use bevy::prelude::Transform;
use serde::{Deserialize, Serialize};

/// Version of the on-disk save format. Increment on any incompatible change.
pub(crate) const SAVE_FORMAT_VERSION: u32 = 1;

/// Directory of the single quicksave slot, relative to the workspace root.
const SAVE_DIR: &str = "../../misc/saves";
/// File name of the single quicksave slot.
const SAVE_FILE: &str = "quicksave.ron";

/// Absolute path of the quicksave slot.
///
/// Resolved from the crate manifest, like the assets directory, so it does not
/// depend on the process working directory.
pub(crate) fn save_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(SAVE_DIR)
        .join(SAVE_FILE)
}

/// Stable 64-bit FNV-1a hash. Used for the `GOTHIC.DAT` fingerprint; unlike
/// `DefaultHasher` the result is a fixed on-disk contract.
pub(crate) fn fnv1a_64(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    bytes.iter().fold(OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(PRIME)
    })
}

/// Root of a quicksave.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SaveGame {
    pub(crate) format_version: u32,
    pub(crate) script_fingerprint: u64,
    pub(crate) world_name: String,
    pub(crate) game_time: SavedGameTime,
    pub(crate) player_transform: Transform,
    pub(crate) player_instance: Option<i32>,
    pub(crate) globals: Vec<SavedValues>,
    pub(crate) instances: Vec<SavedValues>,
    pub(crate) npc_runtimes: Vec<SavedNpcRuntime>,
    pub(crate) item_runtimes: Vec<SavedItemRuntime>,
    pub(crate) journal: Vec<SavedJournalTopic>,
    pub(crate) guild_attitudes: Vec<SavedGuildAttitude>,
    pub(crate) mob_routines: Vec<SavedMobRoutine>,
    pub(crate) live_npcs: Vec<SavedNpcEntity>,
    pub(crate) live_items: Vec<SavedItemEntity>,
    pub(crate) chests: Vec<SavedChest>,
}

impl SaveGame {
    /// Checks the format version and the loaded script fingerprint.
    pub(crate) fn validate(&self, script_fingerprint: u64) -> Result<(), String> {
        if self.format_version != SAVE_FORMAT_VERSION {
            return Err(format!(
                "save format version {} does not match {}",
                self.format_version, SAVE_FORMAT_VERSION
            ));
        }
        if self.script_fingerprint != script_fingerprint {
            return Err(format!(
                "save script fingerprint {:#x} does not match {:#x}",
                self.script_fingerprint, script_fingerprint
            ));
        }
        Ok(())
    }
}

/// Saved in-game clock.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedGameTime {
    pub(crate) total_hours: f32,
    pub(crate) speed: f32,
    pub(crate) day: u32,
}

/// Raw words of one global symbol or one instance record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedValues {
    pub(crate) id: i32,
    pub(crate) name: String,
    pub(crate) values: Vec<i32>,
}

/// Mutable runtime state of one npc instance. Routine state is intentionally
/// absent and is rebuilt by instance initialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SavedNpcRuntime {
    pub(crate) instance_id: i32,
    pub(crate) visual: SavedNpcVisual,
    pub(crate) equipment: SavedNpcEquipment,
    pub(crate) dialog: SavedNpcDialog,
    pub(crate) inventory: Vec<SavedInventoryItem>,
    pub(crate) talent_skills: Vec<i32>,
    pub(crate) talent_values: Vec<i32>,
    pub(crate) social: SavedNpcSocial,
    pub(crate) live: SavedNpcLive,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SavedNpcVisual {
    pub(crate) initialized: bool,
    pub(crate) body_model_id: Option<i32>,
    pub(crate) head_model_id: Option<i32>,
    pub(crate) hierarchy_id: Option<i32>,
    pub(crate) body_texture_index: i32,
    pub(crate) face_texture_index: i32,
    pub(crate) body_model: String,
    pub(crate) body_texture: Option<String>,
    pub(crate) head_model: Option<String>,
    pub(crate) face_texture: Option<String>,
    pub(crate) hierarchy: Option<String>,
    pub(crate) model_scale: Option<[f32; 3]>,
    pub(crate) model_fatness: Option<f32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SavedNpcEquipment {
    pub(crate) melee: Option<i32>,
    pub(crate) ranged: Option<i32>,
    pub(crate) armor: Option<i32>,
    pub(crate) weapon_mode: Option<SavedWeaponMode>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) enum SavedWeaponMode {
    Fists,
    Melee,
    Ranged,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SavedNpcDialog {
    pub(crate) roots: Option<Vec<SavedDialogRoot>>,
    pub(crate) known_infos: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedDialogRoot {
    pub(crate) info_instance_id: Option<i32>,
    pub(crate) sequence_function_id: i32,
    pub(crate) condition_function_id: Option<i32>,
    pub(crate) order: i32,
    pub(crate) description: String,
    pub(crate) important: bool,
    pub(crate) permanent: bool,
    pub(crate) trade: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SavedNpcSocial {
    pub(crate) attitude: Option<i32>,
    pub(crate) temp_attitude: Option<i32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SavedNpcLive {
    pub(crate) position: Option<[f32; 3]>,
    pub(crate) look_at: Option<i32>,
    pub(crate) target: Option<i32>,
}

/// One stack in an npc inventory or a chest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedInventoryItem {
    pub(crate) item: i32,
    pub(crate) count: i32,
}

/// Mutable runtime state of one item instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedItemRuntime {
    pub(crate) instance_id: i32,
    pub(crate) name: String,
    pub(crate) visual: String,
    pub(crate) visual_change: Option<String>,
}

/// One journal topic in creation order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedJournalTopic {
    pub(crate) name: String,
    pub(crate) section: SavedJournalSection,
    pub(crate) status: SavedJournalStatus,
    pub(crate) entries: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) enum SavedJournalSection {
    Mission,
    Note,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) enum SavedJournalStatus {
    Running,
    Success,
    Failed,
    Obsolete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedGuildAttitude {
    pub(crate) guild_a: i32,
    pub(crate) guild_b: i32,
    pub(crate) attitude: i32,
}

/// One `Wld_SetMobRoutine` entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedMobRoutine {
    pub(crate) hour: i32,
    pub(crate) minute: i32,
    pub(crate) mob_name: String,
    pub(crate) state: i32,
}

/// A live npc entity, spawned back at its exact transform on load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedNpcEntity {
    pub(crate) instance_id: i32,
    pub(crate) transform: Transform,
    pub(crate) overlay: Option<String>,
}

/// A live item entity, spawned back at its exact transform on load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedItemEntity {
    pub(crate) instance_id: i32,
    pub(crate) transform: Transform,
}

/// Remaining loot of one chest, keyed by its ZEN container index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedChest {
    pub(crate) index: usize,
    pub(crate) items: Vec<SavedInventoryItem>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_matches_known_values() {
        assert_eq!(fnv1a_64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a_64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a_64(b"hello"), 0xa430_d846_80aa_bd0b);
    }

    fn minimal_save() -> SaveGame {
        SaveGame {
            format_version: SAVE_FORMAT_VERSION,
            script_fingerprint: 42,
            world_name: "NEWWORLD/NEWWORLD".to_string(),
            game_time: SavedGameTime {
                total_hours: 12.0,
                speed: 1.0,
                day: 1,
            },
            player_transform: Transform::IDENTITY,
            player_instance: Some(7),
            globals: vec![SavedValues {
                id: 3,
                name: "test_global".to_string(),
                values: vec![1, 2, 3],
            }],
            instances: Vec::new(),
            npc_runtimes: Vec::new(),
            item_runtimes: Vec::new(),
            journal: vec![SavedJournalTopic {
                name: "TEST_TOPIC".to_string(),
                section: SavedJournalSection::Mission,
                status: SavedJournalStatus::Running,
                entries: vec!["entry".to_string()],
            }],
            guild_attitudes: Vec::new(),
            mob_routines: Vec::new(),
            live_npcs: Vec::new(),
            live_items: Vec::new(),
            chests: Vec::new(),
        }
    }

    #[test]
    fn save_round_trips_through_ron() {
        let save = minimal_save();
        let text = ron::ser::to_string_pretty(&save, ron::ser::PrettyConfig::default())
            .expect("serialize");
        let restored: SaveGame = ron::from_str(&text).expect("deserialize");
        assert_eq!(restored.format_version, save.format_version);
        assert_eq!(restored.script_fingerprint, save.script_fingerprint);
        assert_eq!(restored.world_name, save.world_name);
        assert_eq!(restored.globals.len(), 1);
        assert_eq!(restored.globals[0].values, vec![1, 2, 3]);
        assert_eq!(restored.journal[0].entries, vec!["entry".to_string()]);
    }

    #[test]
    fn validate_rejects_version_and_fingerprint_mismatch() {
        let mut save = minimal_save();
        assert!(save.validate(42).is_ok());

        save.format_version = SAVE_FORMAT_VERSION + 1;
        assert!(save.validate(42).is_err());

        save.format_version = SAVE_FORMAT_VERSION;
        assert!(save.validate(43).is_err());
    }
}
