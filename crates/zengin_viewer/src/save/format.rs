//! On-disk save game format and file IO.
//!
//! The save is a single RON document. Plain runtime values are serialized
//! directly; only the ECS-side payloads and the raw script word containers get
//! dedicated types here.

use std::path::PathBuf;

use bevy::prelude::Transform;
use serde::{Deserialize, Serialize};

use crate::gameplay::game_time::GameTime;
use crate::zengin::InstanceId;
use crate::zengin::JournalTopic;
use crate::zengin::{ItemRuntime, MobRoutineEntry, NpcRuntime};

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
    pub(crate) game_time: GameTime,
    pub(crate) player_transform: Transform,
    pub(crate) script: ScriptSnapshot,
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

/// Script-owned part of a save, collected under one script lock.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ScriptSnapshot {
    pub(crate) player_instance: Option<InstanceId>,
    pub(crate) globals: Vec<SavedValues>,
    pub(crate) instances: Vec<SavedValues>,
    /// The routine field of `NpcRuntime` is skipped by serde and rebuilt on
    /// load, so only mutable non-routine state is persisted.
    pub(crate) npc_runtimes: Vec<(InstanceId, NpcRuntime)>,
    pub(crate) item_runtimes: Vec<(InstanceId, ItemRuntime)>,
    pub(crate) journal: Vec<JournalTopic>,
    pub(crate) guild_attitudes: Vec<SavedGuildAttitude>,
    pub(crate) mob_routines: Vec<MobRoutineEntry>,
}

/// Raw words of one global symbol or one instance record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedValues {
    pub(crate) id: i32,
    pub(crate) name: String,
    pub(crate) values: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedGuildAttitude {
    pub(crate) guild_a: i32,
    pub(crate) guild_b: i32,
    pub(crate) attitude: i32,
}

/// A live npc entity, spawned back at its exact transform on load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedNpcEntity {
    pub(crate) instance_id: InstanceId,
    pub(crate) transform: Transform,
    pub(crate) overlay: Option<String>,
}

/// A live item entity, spawned back at its exact transform on load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedItemEntity {
    pub(crate) instance_id: InstanceId,
    pub(crate) transform: Transform,
}

/// Remaining loot of one chest, keyed by its ZEN container index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedChest {
    pub(crate) index: usize,
    pub(crate) items: Vec<(InstanceId, i32)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zengin::{JournalSection, JournalStatus};

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
            game_time: GameTime::default(),
            player_transform: Transform::IDENTITY,
            script: ScriptSnapshot {
                player_instance: InstanceId::new(7),
                globals: vec![SavedValues {
                    id: 3,
                    name: "test_global".to_string(),
                    values: vec![1, 2, 3],
                }],
                journal: vec![JournalTopic {
                    name: "TEST_TOPIC".to_string(),
                    section: JournalSection::Mission,
                    status: JournalStatus::Running,
                    entries: vec!["entry".to_string()],
                }],
                ..ScriptSnapshot::default()
            },
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
        assert_eq!(restored.script.globals.len(), 1);
        assert_eq!(restored.script.globals[0].values, vec![1, 2, 3]);
        assert_eq!(
            restored.script.journal[0].entries,
            vec!["entry".to_string()]
        );
    }

    #[test]
    fn validate_rejects_version_and_fingerprint_mismatch() {
        let mut save = minimal_save();
        save.validate(42).expect("matching save should validate");

        save.format_version = SAVE_FORMAT_VERSION + 1;
        assert!(save.validate(42).is_err());

        save.format_version = SAVE_FORMAT_VERSION;
        assert!(save.validate(43).is_err());
    }

    #[test]
    fn npc_runtime_round_trips_without_routine_state() {
        let instance = InstanceId::new(5).expect("instance id");
        let item = InstanceId::new(9).expect("item id");
        let mut runtime = NpcRuntime::default();
        runtime.routine.state_time = 123;
        runtime.inventory.create(item, 2);
        runtime.talents.set_skill(0, 3);
        runtime.live.position = Some(bevy::math::Vec3::new(1.0, 2.0, 3.0));

        let mut save = minimal_save();
        save.script.npc_runtimes.push((instance, runtime));
        let text = ron::ser::to_string_pretty(&save, ron::ser::PrettyConfig::default())
            .expect("serialize");
        let restored: SaveGame = ron::from_str(&text).expect("deserialize");

        let (restored_instance, restored) = &restored.script.npc_runtimes[0];
        assert_eq!(*restored_instance, instance);
        assert_eq!(restored.routine.state_time, 0);
        assert_eq!(restored.inventory.count(item), 2);
        assert_eq!(restored.talents.skill(0), 3);
        assert_eq!(
            restored.live.position,
            Some(bevy::math::Vec3::new(1.0, 2.0, 3.0))
        );
    }
}
