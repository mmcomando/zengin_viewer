use bevy::prelude::*;

pub struct StatisticsPlugin;
impl Plugin for StatisticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Statistics>();
        app.add_systems(Update, mesh_added);
    }
}

#[derive(Resource, Default)]
pub struct Statistics {
    pub meshes: i64,
    pub meshes_visible: i64,
}

fn mesh_added(mut statistics: ResMut<Statistics>, query: Query<&Visibility, With<Mesh3d>>) {
    statistics.meshes = 0;
    statistics.meshes_visible = 0;
    for visibility in &query {
        statistics.meshes += 1;
        statistics.meshes_visible += i64::from(*visibility != Visibility::Hidden);
    }
}
