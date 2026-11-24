//! On-disk save game format.
//!
//! A save is a directory of RON documents sharing one [`SaveMetadata`]. Plain
//! runtime values are serialized directly; only the ECS-side payloads, the raw
//! script word containers and the per-file wrappers are dedicated types here.

use std::path::PathBuf;

use bevy::prelude::Transform;
use serde::{Deserialize, Serialize};

use crate::gameplay::game_time::GameTime;
use crate::zengin::InstanceId;
use crate::zengin::JournalTopic;
use crate::zengin::{ItemRuntime, MobRoutineEntry, NpcRuntime};

/// Version of the on-disk save format. Increment on any incompatible change.
pub(crate) const SAVE_FORMAT_VERSION: u32 = 2;

/// Directory containing all saves, relative to the workspace root.
const SAVE_DIR: &str = "../../misc/saves";

/// Name of the metadata document inside a save directory.
pub(crate) const METADATA_FILE: &str = "metadata.ron";
/// Name of the thumbnail inside a save directory.
pub(crate) const SCREENSHOT_FILE: &str = "screenshot.png";
/// Name of the player document inside a save directory.
pub(crate) const PLAYER_FILE: &str = "player.ron";
/// Name of the world document inside a save directory.
pub(crate) const WORLD_FILE: &str = "world.ron";
/// Name of the npc document inside a save directory.
pub(crate) const NPCS_FILE: &str = "npcs.ron";
/// Name of the item document inside a save directory.
pub(crate) const ITEMS_FILE: &str = "items.ron";
/// Name of the journal document inside a save directory.
pub(crate) const JOURNAL_FILE: &str = "journal.ron";
/// Name of the global-script-words document inside a save directory.
pub(crate) const GLOBALS_FILE: &str = "script_globals.ron";
/// Name of the instance-script-words document inside a save directory.
pub(crate) const INSTANCES_FILE: &str = "script_instances.ron";
/// Name of the guild-attitudes document inside a save directory.
pub(crate) const GUILDS_FILE: &str = "guilds.ron";

/// Absolute path of the save directory.
///
/// Resolved from the crate manifest, like the assets directory, so it does not
/// depend on the process working directory.
pub(crate) fn saves_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(SAVE_DIR)
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

/// Metadata shared by every file of one save. Stored in `metadata.ron` so the
/// load screen can list saves without parsing the larger state documents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SaveMetadata {
    pub(crate) format_version: u32,
    pub(crate) script_fingerprint: u64,
    pub(crate) world_name: String,
    /// Unix timestamp of the save, used in the directory name and the load-screen
    /// panel.
    pub(crate) saved_at_unix: u64,
    pub(crate) game_time: GameTime,
    /// Real time played in this save, in seconds.
    pub(crate) play_time_seconds: f64,
    /// Thumbnail file name inside the save directory, `None` when no screenshot
    /// could be captured.
    pub(crate) screenshot: Option<String>,
}

impl SaveMetadata {
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

    /// `YYYY-MM-DD - HH:MM` in UTC, matching the load screen's saved-at field.
    pub(crate) fn saved_at_label(&self) -> String {
        format_unix_utc_minutes(self.saved_at_unix)
    }

    /// `Day: N  Time: HH:MM` of the in-game clock.
    pub(crate) fn game_time_label(&self) -> String {
        format_game_time(self.game_time)
    }

    /// `Xh Ymin` of real play time.
    pub(crate) fn play_time_label(&self) -> String {
        format_play_time(self.play_time_seconds)
    }
}

/// In-memory aggregate of one save, assembled from the directory files.
#[derive(Debug, Clone)]
pub(crate) struct SaveGame {
    pub(crate) metadata: SaveMetadata,
    pub(crate) player_transform: Transform,
    pub(crate) script: ScriptSnapshot,
    pub(crate) live_npcs: Vec<SavedNpcEntity>,
    pub(crate) live_items: Vec<SavedItemEntity>,
    pub(crate) chests: Vec<SavedChest>,
}

impl SaveGame {
    /// Checks the format version and the loaded script fingerprint.
    pub(crate) fn validate(&self, script_fingerprint: u64) -> Result<(), String> {
        self.metadata.validate(script_fingerprint)
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

/// Remaining loot of one chest, keyed by the vob id in the ZEN archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedChest {
    pub(crate) vob_id: u32,
    pub(crate) items: Vec<(InstanceId, i32)>,
    /// Defaults to `false` (still locked) when absent from the save.
    #[serde(default)]
    pub(crate) cracked: bool,
}

/// Payload of `player.ron`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PlayerSave {
    pub(crate) player_instance: Option<InstanceId>,
    pub(crate) player_transform: Transform,
}

/// Payload of `world.ron`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorldSave {
    pub(crate) live_npcs: Vec<SavedNpcEntity>,
    pub(crate) live_items: Vec<SavedItemEntity>,
    pub(crate) chests: Vec<SavedChest>,
    pub(crate) mob_routines: Vec<MobRoutineEntry>,
}

/// Payload of `npcs.ron`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NpcsSave {
    pub(crate) npc_runtimes: Vec<(InstanceId, NpcRuntime)>,
}

/// Payload of `items.ron`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ItemsSave {
    pub(crate) item_runtimes: Vec<(InstanceId, ItemRuntime)>,
}

/// Payload of `journal.ron`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JournalSave {
    pub(crate) journal: Vec<JournalTopic>,
}

/// Payload of `script_globals.ron`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GlobalsSave {
    pub(crate) globals: Vec<SavedValues>,
}

/// Payload of `script_instances.ron`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct InstancesSave {
    pub(crate) instances: Vec<SavedValues>,
}

/// Payload of `guilds.ron`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GuildsSave {
    pub(crate) guild_attitudes: Vec<SavedGuildAttitude>,
}

/// Formats a unix timestamp as the timestamp used in save directory names:
/// `YYYY-MM-DD_HH-MM-SS` in UTC.
pub(crate) fn format_unix_utc(seconds: u64) -> String {
    let (date, hour, minute, second) = split_unix_utc(seconds);
    format!("{date}_{hour:02}-{minute:02}-{second:02}")
}

/// Formats a unix timestamp for the load screen: `YYYY-MM-DD - HH:MM` in UTC.
pub(crate) fn format_unix_utc_minutes(seconds: u64) -> String {
    let (date, hour, minute, _second) = split_unix_utc(seconds);
    format!("{date} - {hour:02}:{minute:02}")
}

/// Formats the in-game clock as `Day: N  Time: HH:MM`.
pub(crate) fn format_game_time(game_time: GameTime) -> String {
    let minutes = game_time.minutes_of_day();
    format!(
        "Day: {}  Time: {:02}:{:02}",
        game_time.day,
        minutes / 60,
        minutes % 60
    )
}

/// Formats real play time as `Xh Ymin`.
pub(crate) fn format_play_time(seconds: f64) -> String {
    let total_minutes = (seconds.max(0.0) / 60.0) as u64;
    format!("{}h {}min", total_minutes / 60, total_minutes % 60)
}

/// Splits a unix timestamp into `(YYYY-MM-DD, hour, minute, second)` in UTC.
fn split_unix_utc(seconds: u64) -> (String, u64, u64, u64) {
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    (
        format!("{year:04}-{month:02}-{day:02}"),
        hour,
        minute,
        second,
    )
}

/// Converts days since 1970-01-01 into a proleptic Gregorian calendar date.
/// Based on Howard Hinnant's `civil_from_days` algorithm.
fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let shifted = days_since_epoch + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    (year, month as u32, day as u32)
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

    fn metadata() -> SaveMetadata {
        SaveMetadata {
            format_version: SAVE_FORMAT_VERSION,
            script_fingerprint: 42,
            world_name: "NEWWORLD/NEWWORLD".to_string(),
            saved_at_unix: 1_762_957_800,
            game_time: GameTime {
                total_hours: 13.75,
                speed: 1.0,
                day: 49,
            },
            play_time_seconds: 28.0 * 3_600.0 + 54.0 * 60.0,
            screenshot: Some(SCREENSHOT_FILE.to_string()),
        }
    }

    #[test]
    fn metadata_round_trips_through_ron() {
        let metadata = metadata();
        let text = ron::ser::to_string_pretty(&metadata, ron::ser::PrettyConfig::default())
            .expect("serialize");
        let restored: SaveMetadata = ron::from_str(&text).expect("deserialize");

        assert_eq!(restored.format_version, metadata.format_version);
        assert_eq!(restored.script_fingerprint, metadata.script_fingerprint);
        assert_eq!(restored.world_name, metadata.world_name);
        assert_eq!(restored.saved_at_unix, metadata.saved_at_unix);
        assert_eq!(restored.play_time_seconds, metadata.play_time_seconds);
        assert_eq!(restored.screenshot, metadata.screenshot);
    }

    #[test]
    fn validate_rejects_version_and_fingerprint_mismatch() {
        let mut metadata = metadata();
        metadata
            .validate(42)
            .expect("matching metadata should validate");

        metadata.format_version = SAVE_FORMAT_VERSION + 1;
        assert!(metadata.validate(42).is_err());

        metadata.format_version = SAVE_FORMAT_VERSION;
        assert!(metadata.validate(43).is_err());
    }

    #[test]
    fn load_panel_labels_match_the_real_game_style() {
        let metadata = metadata();
        assert_eq!(metadata.saved_at_label(), "2025-11-12 - 14:30");
        assert_eq!(metadata.game_time_label(), "Day: 49  Time: 13:45");
        assert_eq!(metadata.play_time_label(), "28h 54min");
    }

    #[test]
    fn filename_and_minute_timestamps_agree() {
        assert_eq!(format_unix_utc(0), "1970-01-01_00-00-00");
        assert_eq!(format_unix_utc_minutes(1_762_957_800), "2025-11-12 - 14:30");
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

        let part = NpcsSave {
            npc_runtimes: vec![(instance, runtime)],
        };
        let text = ron::ser::to_string_pretty(&part, ron::ser::PrettyConfig::default())
            .expect("serialize");
        let restored: NpcsSave = ron::from_str(&text).expect("deserialize");

        let (restored_instance, restored) = &restored.npc_runtimes[0];
        assert_eq!(*restored_instance, instance);
        assert_eq!(restored.routine.state_time, 0);
        assert_eq!(restored.inventory.count(item), 2);
        assert_eq!(restored.talents.skill(0), 3);
        assert_eq!(
            restored.live.position,
            Some(bevy::math::Vec3::new(1.0, 2.0, 3.0))
        );
    }

    #[test]
    fn journal_topic_round_trips() {
        let part = JournalSave {
            journal: vec![JournalTopic {
                name: "TEST_TOPIC".to_string(),
                section: JournalSection::Mission,
                status: JournalStatus::Running,
                entries: vec!["entry".to_string()],
            }],
        };
        let text = ron::ser::to_string_pretty(&part, ron::ser::PrettyConfig::default())
            .expect("serialize");
        let restored: JournalSave = ron::from_str(&text).expect("deserialize");
        assert_eq!(restored.journal[0].entries, vec!["entry".to_string()]);
    }
}
