use bevy::prelude::*;

use crate::amortized_executor::AmortizedExecutor;
use crate::perf_debug;

/// Registers [`AmortizedExecutor`] and advances it every frame while
/// `active_state` is active.
pub struct AmortizedExecutorPlugin<S: States> {
    /// Start without a per-frame budget so every due callback runs each frame.
    pub unlimited: bool,
    /// State in which the executor advances.
    pub active_state: S,
}

impl<S: States> Plugin for AmortizedExecutorPlugin<S> {
    fn build(&self, app: &mut App) {
        let executor = if self.unlimited {
            AmortizedExecutor::new_unlimited()
        } else {
            AmortizedExecutor::default()
        };
        app.insert_resource(AmortizedExecutorResource(executor));
        app.add_systems(
            Update,
            run_executor.run_if(in_state(self.active_state.clone())),
        );
    }
}

/// Bevy resource wrapper around [`AmortizedExecutor`], which itself stays
/// independent of the engine.
#[derive(Resource, Deref, DerefMut)]
pub(crate) struct AmortizedExecutorResource(pub(crate) AmortizedExecutor);

fn run_executor(time: Res<Time>, mut executor: ResMut<AmortizedExecutorResource>) {
    let _perf = perf_debug!();
    let delta_us = time.delta().as_micros() as u64;
    executor.execute(delta_us);
}
