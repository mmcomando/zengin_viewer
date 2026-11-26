use bevy::prelude::*;
use dev_util::perf_debug;

/// Requests a recount of the debug statistics. The scans are expensive, so
/// they only run when the debug UI asks for them.
#[derive(Message)]
pub struct RefreshStatistics;

pub struct StatisticsPlugin;
impl Plugin for StatisticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Statistics>();
        app.add_message::<RefreshStatistics>();
        app.add_systems(Update, mesh_added.run_if(on_message::<RefreshStatistics>));
    }
}

#[derive(Resource, Default)]
pub struct Statistics {
    pub meshes: i64,
    pub meshes_visible: i64,
}

fn mesh_added(mut statistics: ResMut<Statistics>, query: Query<&Visibility, With<Mesh3d>>) {
    let _perf = perf_debug!();
    statistics.meshes = 0;
    statistics.meshes_visible = 0;
    for visibility in &query {
        statistics.meshes += 1;
        statistics.meshes_visible += i64::from(*visibility != Visibility::Hidden);
    }
}
