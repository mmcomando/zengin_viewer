//! Message types whose producer runs outside the ECS. A producer clone of
//! [`ExternalMessageSender`] appends to a shared inbox from any thread or
//! callback, and a pump system copies the inbox into the regular Bevy message
//! buffers once per frame, so consumers keep reading with `MessageReader`.

use bevy::ecs::message::{MessageUpdateSystems, MessageWriter};
use bevy::prelude::*;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Queue handle for a message produced outside the ECS. Clones share the queue.
pub struct ExternalMessageSender<M: Message> {
    inbox: Arc<Mutex<Vec<M>>>,
}

impl<M: Message> Clone for ExternalMessageSender<M> {
    fn clone(&self) -> Self {
        Self {
            inbox: Arc::clone(&self.inbox),
        }
    }
}

impl<M: Message> ExternalMessageSender<M> {
    /// Queues `message` for the next pump. Readers see it no earlier than the
    /// next `First` pass, so the delay depends on how long the caller holds the
    /// handle before the frame boundary.
    pub fn send(&self, message: M) {
        self.lock().push(message);
    }

    /// Drops every message the pump has not forwarded yet.
    pub fn clear(&self) {
        self.lock().clear();
    }

    /// Removes and returns the messages queued so far, for tests that drive the
    /// producer without an app.
    #[cfg(test)]
    pub fn take(&self) -> Vec<M> {
        std::mem::take(&mut self.lock())
    }

    fn lock(&self) -> MutexGuard<'_, Vec<M>> {
        self.inbox.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A default sender owns a queue no pump reads, so everything sent through it is
/// dropped. Build senders with [`ExternalMessageAppExt::add_external_message`]
/// unless the messages are meant to go nowhere.
impl<M: Message> Default for ExternalMessageSender<M> {
    fn default() -> Self {
        Self {
            inbox: Arc::default(),
        }
    }
}

/// Receiving half of an [`ExternalMessageSender`] queue.
#[derive(Resource)]
pub struct ExternalMessageInbox<M: Message>(Arc<Mutex<Vec<M>>>);

/// Registers message types that are pushed from outside the ECS.
pub trait ExternalMessageAppExt {
    /// Same as [`App::add_message`], plus a handle used to queue the message
    /// from code that has no world access.
    fn add_external_message<M: Message>(&mut self) -> ExternalMessageSender<M>;
}

impl ExternalMessageAppExt for App {
    fn add_external_message<M: Message>(&mut self) -> ExternalMessageSender<M> {
        let inbox = Arc::new(Mutex::new(Vec::new()));
        self.add_message::<M>()
            .insert_resource(ExternalMessageInbox::<M>(Arc::clone(&inbox)))
            // Before the buffer rotation, so a message queued from another
            // thread before this frame starts is readable in the same frame.
            .add_systems(
                First,
                pump_external_messages::<M>.before(MessageUpdateSystems),
            );
        ExternalMessageSender { inbox }
    }
}

/// Forwards the queued messages into `Messages<M>`.
///
/// [`add_external_message`](ExternalMessageAppExt::add_external_message) already
/// schedules this in `First`. Schedule it a second time, ordered between the
/// producer and the consumer, when the consumer must see a message in the same
/// frame it was produced (the dialog line playback is such a consumer).
pub fn pump_external_messages<M: Message>(
    inbox: Res<ExternalMessageInbox<M>>,
    mut messages: MessageWriter<M>,
) {
    let queued = std::mem::take(&mut *inbox.0.lock().unwrap_or_else(PoisonError::into_inner));
    // Empty batches would still mark `Messages<M>` as changed and defeat the
    // registry's skip of the buffer rotation.
    if queued.is_empty() {
        return;
    }
    messages.write_batch(queued);
}

#[cfg(test)]
mod tests {
    use super::ExternalMessageAppExt;
    use bevy::prelude::*;
    use std::sync::{Arc, Mutex, PoisonError};

    #[derive(Message, Debug, PartialEq, Eq)]
    struct TestMessage(u32);

    #[test]
    fn queued_messages_reach_readers() {
        let mut app = App::new();
        let sender = app.add_external_message::<TestMessage>();
        let seen = Arc::new(Mutex::new(Vec::new()));
        {
            let seen = Arc::clone(&seen);
            app.add_systems(Update, move |mut reader: MessageReader<TestMessage>| {
                let mut seen = seen.lock().unwrap_or_else(PoisonError::into_inner);
                seen.extend(reader.read().map(|message| message.0));
            });
        }

        sender.send(TestMessage(1));
        app.update();
        assert_eq!(*seen.lock().unwrap_or_else(PoisonError::into_inner), [1]);

        sender.send(TestMessage(2));
        app.update();
        sender.send(TestMessage(3));
        app.update();
        assert_eq!(
            *seen.lock().unwrap_or_else(PoisonError::into_inner),
            [1, 2, 3]
        );
    }
}
