use bevy::prelude::*;

use crate::zengin::{JournalSection, JournalStatus, JournalTopic};

/// Sections of the journal, matching the original category widgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum JournalCategory {
    /// `CURRENTMISSIONS`.
    #[default]
    Current,
    /// `OLDMISSIONS`.
    Completed,
    /// `FAILEDMISSIONS`.
    Failed,
    /// `LOG` — notes.
    General,
}

impl JournalCategory {
    pub(crate) const ALL: [Self; 4] = [Self::Current, Self::Completed, Self::Failed, Self::General];

    /// Original menu labels in the Polish game data; they move into the menu
    /// script once menu data is loaded.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Current => "Obecne zadania",
            Self::Completed => "Wykonane zadania",
            Self::Failed => "Popsute zadania",
            Self::General => "Informacje ogólne",
        }
    }

    fn matches(self, topic: &JournalTopic) -> bool {
        match self {
            Self::Current => {
                topic.section == JournalSection::Mission && topic.status == JournalStatus::Running
            }
            Self::Completed => {
                topic.section == JournalSection::Mission
                    && matches!(
                        topic.status,
                        JournalStatus::Success | JournalStatus::Obsolete
                    )
            }
            Self::Failed => {
                topic.section == JournalSection::Mission && topic.status == JournalStatus::Failed
            }
            Self::General => topic.section == JournalSection::Note,
        }
    }
}

/// Which column receives the navigation input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum JournalFocus {
    #[default]
    Categories,
    Topics,
}

/// State of the journal UI: closed by default, topics snapshot on open.
#[derive(Resource, Default)]
pub(crate) struct JournalUiState {
    pub(crate) open: bool,
    pub(crate) category: JournalCategory,
    pub(crate) focus: JournalFocus,
    /// Selected entry in the filtered topic list.
    pub(crate) selected: usize,
    /// Indices into `topics` matching `category`, newest first.
    pub(crate) active: Vec<usize>,
    pub(crate) topics: Vec<JournalTopic>,
    pub(crate) detail_open: bool,
    /// Scroll offset of the entry viewer in text lines.
    pub(crate) detail_scroll: usize,
    pub(crate) detail_dirty: bool,
    pub(crate) rows_dirty: bool,
    pub(crate) reset_scroll: bool,
}

impl JournalUiState {
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    pub(crate) fn close(&mut self) {
        *self = Self::default();
    }

    /// Steps back from the entry viewer instead of closing the journal.
    /// Returns `false` when the journal itself was closed.
    pub(crate) fn escape(&mut self) -> bool {
        if self.detail_open {
            self.detail_open = false;
            self.detail_scroll = 0;
            return true;
        }
        self.close();
        false
    }

    pub(crate) fn open(&mut self, topics: Vec<JournalTopic>) {
        *self = Self {
            open: true,
            topics,
            rows_dirty: true,
            reset_scroll: true,
            ..default()
        };
        self.rebuild_active();
    }

    fn select_category(&mut self, category: JournalCategory) {
        if self.category == category {
            return;
        }
        self.category = category;
        self.rebuild_active();
        self.rows_dirty = true;
        self.reset_scroll = true;
    }

    pub(crate) fn move_category(&mut self, delta: isize) {
        let index = JournalCategory::ALL
            .iter()
            .position(|category| *category == self.category)
            .unwrap_or(0);
        let next = (index as isize + delta).clamp(0, JournalCategory::ALL.len() as isize - 1);
        self.select_category(JournalCategory::ALL[next as usize]);
    }

    fn rebuild_active(&mut self) {
        self.active.clear();
        for (index, topic) in self.topics.iter().enumerate().rev() {
            if self.category.matches(topic) {
                self.active.push(index);
            }
        }
        self.selected = 0;
    }

    fn current_topic(&self) -> Option<&JournalTopic> {
        let index = *self.active.get(self.selected)?;
        self.topics.get(index)
    }

    pub(crate) fn detail_text(&self) -> String {
        let Some(topic) = self.current_topic() else {
            return String::new();
        };
        topic.entries.join("\n---\n")
    }
}
