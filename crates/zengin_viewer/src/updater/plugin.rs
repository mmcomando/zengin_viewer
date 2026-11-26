use bevy::prelude::*;

use crate::game_state::AppState;
use crate::input::Options;

use crate::perf_debug;
use dev_util::amortized_executor::AmortizedExecutor;

/// Registers [`AmortizedExecutor`] and advances it every frame.
pub(crate) struct AmortizedExecutorPlugin;

impl Plugin for AmortizedExecutorPlugin {
    fn build(&self, app: &mut App) {
        let executor = if app.world().resource::<Options>().scenario_enabled() {
            AmortizedExecutor::new_unlimited()
        } else {
            AmortizedExecutor::default()
        };
        app.insert_resource(AmortizedExecutorResource(executor));
        app.add_systems(Update, run_executor.run_if(in_state(AppState::InGame)));
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
