use super::*;

impl mmcomando::zengine_scripts::log_scripts::Host for ImportCallState {
    /// Creates a journal topic in the given section (`Log_CreateTopic`).
    fn log_createtopic(&mut self, topic: i32, section: i32) {
        let Some(topic) = self.get_string_non_0(topic) else {
            return;
        };
        let Some(section) = JournalSection::from_i32(section) else {
            return;
        };
        if self.journal.create_topic(topic.clone(), section) {
            self.pending.journal_changes.push(JournalChange {
                topic,
                action: JournalAction::Created { section },
            });
        }
    }
    /// Appends an entry to a journal topic (`Log_AddEntry`).
    fn log_addentry(&mut self, topic: i32, entry: i32) {
        let Some(topic) = self.get_string_non_0(topic) else {
            return;
        };
        let Some(entry) = self.get_string_non_0(entry) else {
            return;
        };
        if self.journal.add_entry(&topic, entry.clone()) {
            self.pending.journal_changes.push(JournalChange {
                topic,
                action: JournalAction::Entry(entry),
            });
        }
    }
    /// Sets the status of a journal topic (`Log_SetTopicStatus`).
    fn log_settopicstatus(&mut self, topic: i32, status: i32) {
        let Some(topic) = self.get_string_non_0(topic) else {
            return;
        };
        let Some(status) = JournalStatus::from_i32(status) else {
            return;
        };
        if self.journal.set_topic_status(&topic, status) {
            self.pending.journal_changes.push(JournalChange {
                topic,
                action: JournalAction::Status(status),
            });
        }
    }
}
