use bevy::prelude::*;

use crate::{game::objects::GameNpc, zengin::zengin_resources::ZenGinModelComponent};

pub struct StatisticsPlugin;
impl Plugin for StatisticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Statistics>();
        app.add_systems(Update, mesh_added);
        app.add_systems(Update, zengin_model_added);
        app.add_systems(Update, npc_added);
    }
}

#[derive(Resource, Default)]
pub struct Statistics {
    pub meshes: i64,
    pub meshes_visible: i64,
    pub zengin_models: i64,
    pub zengin_models_visible: i64,
    pub npcs: i64,
    pub npcs_visible: i64,
}

fn mesh_added(mut statistics: ResMut<Statistics>, query: Query<&Visibility, With<Mesh3d>>) {
    statistics.meshes = 0;
    statistics.meshes_visible = 0;
    for visibility in &query {
        statistics.meshes += 1;
        statistics.meshes_visible += (*visibility != Visibility::Hidden) as i64;
    }
}

fn zengin_model_added(
    mut statistics: ResMut<Statistics>,
    query: Query<&Visibility, With<ZenGinModelComponent>>,
) {
    statistics.zengin_models = 0;
    statistics.zengin_models_visible = 0;
    for visibility in &query {
        statistics.zengin_models += 1;
        statistics.zengin_models_visible += (*visibility != Visibility::Hidden) as i64;
    }
}

fn npc_added(mut statistics: ResMut<Statistics>, query: Query<&Visibility, With<GameNpc>>) {
    statistics.npcs = 0;
    statistics.npcs_visible = 0;
    for visibility in &query {
        statistics.npcs += 1;
        statistics.npcs_visible += (*visibility != Visibility::Hidden) as i64;
    }
}
