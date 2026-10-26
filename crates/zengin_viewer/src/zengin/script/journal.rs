//! Journal (quest log) state, populated by the `Log_*` host functions. Topics
//! are identified by their script-side name string.

use bevy::prelude::*;

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
#[derive(Debug, Clone)]
pub struct JournalTopic {
    pub name: String,
    pub section: JournalSection,
    pub status: JournalStatus,
    /// Entries in insertion order.
    pub entries: Vec<String>,
}

/// All journal topics in creation order.
#[derive(Debug, Default)]
pub struct JournalManager {
    topics: Vec<JournalTopic>,
}

impl JournalManager {
    /// Topics in creation order, oldest first.
    pub fn topics(&self) -> &[JournalTopic] {
        &self.topics
    }

    /// Adds a new topic in `section`. Returns `false` when a topic with that
    /// name already exists.
    pub fn create_topic(&mut self, name: String, section: JournalSection) -> bool {
        if self.find(&name).is_some() {
            return false;
        }
        self.topics.push(JournalTopic {
            name,
            section,
            status: JournalStatus::Running,
            entries: Vec::new(),
        });
        true
    }

    /// Sets the status of a topic. A missing mission topic is created unless
    /// the status is [`JournalStatus::Obsolete`]. Returns whether the topic
    /// exists or was created.
    pub fn set_topic_status(&mut self, name: &str, status: JournalStatus) -> bool {
        if let Some(topic) = self.find_mut(name) {
            topic.status = status;
            return true;
        }
        if status == JournalStatus::Obsolete {
            return false;
        }
        self.topics.push(JournalTopic {
            name: name.to_string(),
            section: JournalSection::Mission,
            status,
            entries: Vec::new(),
        });
        true
    }

    /// Appends an entry to a topic. Returns `false` when the topic does not
    /// exist.
    pub fn add_entry(&mut self, name: &str, entry: String) -> bool {
        let Some(topic) = self.find_mut(name) else {
            return false;
        };
        topic.entries.push(entry);
        true
    }

    fn find(&self, name: &str) -> Option<&JournalTopic> {
        self.topics.iter().find(|topic| topic.name == name)
    }

    fn find_mut(&mut self, name: &str) -> Option<&mut JournalTopic> {
        self.topics.iter_mut().find(|topic| topic.name == name)
    }
}
