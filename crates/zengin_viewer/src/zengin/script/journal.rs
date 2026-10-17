//! Journal (quest log) state, populated by the `Log_*` host functions. Topics
//! are identified by their script-side name string.

use std::collections::HashMap;

use bevy::prelude::*;

/// Section of a journal topic (`Log_CreateTopic` argument).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalSection {
    /// `LOG_MISSION` — normal quest.
    Mission,
    /// `LOG_NOTE` — note.
    Note,
}

impl JournalSection {
    /// Script-side value (`LOG_MISSION`/`LOG_NOTE`), `None` otherwise.
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Mission),
            1 => Some(Self::Note),
            _ => None,
        }
    }
}

/// Status of a journal topic (`Log_SetTopicStatus` argument).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalStatus {
    /// `LOG_RUNNING` — active quest.
    Running,
    /// `LOG_SUCCESS` — finished quest.
    Success,
    /// `LOG_FAILED` — failed quest.
    Failed,
    /// `LOG_OBSOLETE` — canceled quest.
    Obsolete,
}

impl JournalStatus {
    /// Script-side value (`LOG_RUNNING`..`LOG_OBSOLETE`), `None` otherwise.
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Running),
            2 => Some(Self::Success),
            3 => Some(Self::Failed),
            4 => Some(Self::Obsolete),
            _ => None,
        }
    }
}

/// One journal topic: its section, status and accumulated entries.
#[derive(Debug)]
pub struct JournalTopic {
    /// Section of the topic, used by the future journal UI.
    #[allow(dead_code)]
    pub section: JournalSection,
    /// `None` until `Log_SetTopicStatus` sets it.
    pub status: Option<JournalStatus>,
    /// Entries in insertion order.
    pub entries: Vec<String>,
}

/// All journal topics, keyed by their name.
#[derive(Debug, Default)]
pub struct JournalManager {
    topics: HashMap<String, JournalTopic>,
}

impl JournalManager {
    /// Adds a new topic in `section`, replacing an existing one. Returns
    /// `false` when the section is neither mission nor note.
    pub fn create_topic(&mut self, name: String, section: JournalSection) -> bool {
        self.topics.insert(
            name,
            JournalTopic {
                section,
                status: None,
                entries: Vec::new(),
            },
        );
        true
    }

    /// Sets the status of a topic. Returns `false` when the topic does not
    /// exist.
    pub fn set_topic_status(&mut self, name: &str, status: JournalStatus) -> bool {
        let Some(topic) = self.topics.get_mut(name) else {
            return false;
        };
        topic.status = Some(status);
        true
    }

    /// Appends an entry to a topic. Returns `false` when the topic does not
    /// exist.
    pub fn add_entry(&mut self, name: &str, entry: String) -> bool {
        let Some(topic) = self.topics.get_mut(name) else {
            return false;
        };
        topic.entries.push(entry);
        true
    }
}

/// What happened to a journal topic, recorded for the game log.
#[derive(Debug, Clone)]
pub enum JournalAction {
    /// `Log_CreateTopic` created the topic.
    Created { section: JournalSection },
    /// `Log_SetTopicStatus` set the given status.
    Status(JournalStatus),
    /// `Log_AddEntry` added the given entry text.
    Entry(String),
}

/// A journal change produced by the script, drained to the ECS by the script
/// bridge for the game log. Topic and entry texts are already resolved.
#[derive(Debug, Message)]
pub struct JournalChange {
    pub topic: String,
    pub action: JournalAction,
}
