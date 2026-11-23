use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use bevy::prelude::*;

use crate::external_messages::{ExternalMessageAppExt, ExternalMessageSender};

use super::executor::{CallbackControl, CallbackHandle};
use super::plugin::AmortizedExecutorResource;
use crate::perf_debug;

/// Marker type selecting one independent stream of periodic updates.
///
/// Each kind gets its own message channel and executor callbacks, so different
/// kinds can be registered and consumed without seeing each other's updates.
pub(crate) trait PeriodicKind: Send + Sync + 'static {}

/// Registers its entity in the `T` update stream, one update per `period`.
#[derive(Component)]
pub(crate) struct Periodic<T: PeriodicKind> {
    period: Duration,
    _kind: PhantomData<fn() -> T>,
}

impl<T: PeriodicKind> Periodic<T> {
    pub(crate) fn new(period: Duration) -> Self {
        Self {
            period,
            _kind: PhantomData,
        }
    }
}

/// One due update for `entity`, queued by the executor for the `T` stream.
#[derive(Message)]
pub(crate) struct PeriodicUpdate<T: PeriodicKind> {
    pub(crate) entity: Entity,
    pub(crate) control: Arc<CallbackControl>,
    _kind: PhantomData<fn() -> T>,
}

/// Sender half of the [`PeriodicUpdate`] queue of one kind.
#[derive(Resource)]
struct PeriodicSender<T: PeriodicKind>(ExternalMessageSender<PeriodicUpdate<T>>);

/// Executor handles of the registered entities, keyed by entity.
#[derive(Resource)]
struct PeriodicHandles<T: PeriodicKind>(HashMap<Entity, CallbackHandle>, PhantomData<fn() -> T>);

impl<T: PeriodicKind> Default for PeriodicHandles<T> {
    fn default() -> Self {
        Self(HashMap::new(), PhantomData)
    }
}

/// Adds the channel and registration systems for one periodic update stream.
pub(crate) trait PeriodicAppExt {
    fn add_periodic_updates<T: PeriodicKind>(&mut self);
}

impl PeriodicAppExt for App {
    fn add_periodic_updates<T: PeriodicKind>(&mut self) {
        if self.world().contains_resource::<PeriodicSender<T>>() {
            return;
        }
        let sender = self.add_external_message::<PeriodicUpdate<T>>();
        self.insert_resource(PeriodicSender::<T>(sender));
        self.init_resource::<PeriodicHandles<T>>();
        self.add_systems(Update, (register_periodic::<T>, remove_periodic::<T>));
    }
}

fn register_periodic<T: PeriodicKind>(
    added: Query<(Entity, &Periodic<T>), Added<Periodic<T>>>,
    mut executor: ResMut<AmortizedExecutorResource>,
    sender: Res<PeriodicSender<T>>,
    mut handles: ResMut<PeriodicHandles<T>>,
) {
    let _perf = perf_debug!();
    for (entity, periodic) in &added {
        if handles.0.contains_key(&entity) {
            continue;
        }
        let callback_sender = sender.0.clone();
        let handle = executor.add(periodic.period, move |control| {
            callback_sender.send(PeriodicUpdate {
                entity,
                control: Arc::clone(control),
                _kind: PhantomData,
            });
        });
        handles.0.insert(entity, handle);
    }
}

fn remove_periodic<T: PeriodicKind>(
    mut removed: RemovedComponents<Periodic<T>>,
    mut executor: ResMut<AmortizedExecutorResource>,
    mut handles: ResMut<PeriodicHandles<T>>,
) {
    let _perf = perf_debug!();
    for entity in removed.read() {
        if let Some(handle) = handles.0.remove(&entity) {
            executor.remove(handle);
        }
    }
}
