use bevy::prelude::*;

#[derive(Default)]
pub struct ToggleVisibility;

impl Plugin for ToggleVisibility {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, insert_resources);

        app.add_systems(Update, toggle_visibility_world_mesh);
        app.add_systems(Update, toggle_visibility_static_meshes);
        app.add_systems(Update, toggle_visibility_npcs);
    }
}

fn insert_resources(mut commands: Commands) {
    commands.insert_resource(ToggleHide::default());
}

#[derive(Component, Default)]
pub struct NpcVisibility {}

#[derive(Component, Default)]
pub struct WorldMesh {}

#[derive(Component, Default)]
pub struct StaticMesh {}

#[derive(Resource, Default)]
struct ToggleHide {
    show_world_mesh: bool,
    show_static_meshes: bool,
    show_npcs: bool,
}

fn toggle_visibility_world_mesh(
    keys: Res<ButtonInput<KeyCode>>,
    mut toggle_info: ResMut<ToggleHide>,
    mut query: Query<(&mut Visibility, &WorldMesh)>,
) {
    if !keys.just_pressed(KeyCode::KeyT) {
        return;
    }
    info!("Toggle world mesh visibility");
    toggle_info.show_world_mesh = !toggle_info.show_world_mesh;
    let vis = if toggle_info.show_world_mesh {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for (mut visibility, _mesh) in &mut query {
        *visibility = vis;
    }
}

fn toggle_visibility_static_meshes(
    keys: Res<ButtonInput<KeyCode>>,
    mut toggle_info: ResMut<ToggleHide>,
    mut query: Query<(&mut Visibility, &StaticMesh)>,
) {
    if !keys.just_pressed(KeyCode::Digit1) {
        return;
    }
    info!("Toggle static meshes visibility");
    toggle_info.show_static_meshes = !toggle_info.show_static_meshes;
    let vis = if toggle_info.show_static_meshes {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for (mut visibility, _mesh) in &mut query {
        *visibility = vis;
    }
}

fn toggle_visibility_npcs(
    keys: Res<ButtonInput<KeyCode>>,
    mut toggle_info: ResMut<ToggleHide>,
    mut query: Query<(&mut Visibility, &NpcVisibility)>,
) {
    if !keys.just_pressed(KeyCode::Digit2) {
        return;
    }
    info!("Toggle npcs visibility");
    toggle_info.show_npcs = !toggle_info.show_npcs;
    let vis = if toggle_info.show_npcs {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for (mut visibility, _mesh) in &mut query {
        *visibility = vis;
    }
}
