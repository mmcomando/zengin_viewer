/// Based on example <https://bevy.org/examples/3d-rendering/skybox>
use bevy::camera_controller::free_camera::FreeCamera;
use bevy::core_pipeline::Skybox;
use bevy::prelude::*;
use bevy::render::render_resource::{TextureViewDescriptor, TextureViewDimension};

use crate::game_state::AppState;

#[derive(Default)]
pub struct SkyBoxPlugin;

impl Plugin for SkyBoxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_skybox);
        app.add_systems(
            Update,
            update_skybox_texture.run_if(in_state(AppState::InGame)),
        );
        app.add_systems(
            Update,
            add_skybox_to_camera
                .run_if(|res: Res<Cubemap>| res.is_loaded)
                .run_if(in_state(AppState::InGame)),
        );
    }
}

#[derive(Resource)]
struct Cubemap {
    is_loaded: bool,
    image_handle: Handle<Image>,
}
// Step 1 Insert Resource and load a skybox texture
fn spawn_skybox(mut commands: Commands, asset_server: Res<AssetServer>) {
    let skybox_handle = asset_server.load("cubemap.png");

    commands.insert_resource(Cubemap {
        is_loaded: false,
        image_handle: skybox_handle.clone(),
    });
}
// Step 2 Make 2d skybox texture be 3d texture
fn update_skybox_texture(
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut cubemap: ResMut<Cubemap>,
) {
    if cubemap.is_loaded {
        return;
    }
    if !asset_server.load_state(&cubemap.image_handle).is_loaded() {
        return;
    }
    let mut image = images.get_mut(&cubemap.image_handle).unwrap();
    // NOTE: PNGs do not have any metadata that could indicate they contain a cubemap texture,
    // so they appear as one texture. The following code reconfigures the texture as necessary.
    if image.texture_descriptor.array_layer_count() == 1 {
        let layers = image.height() / image.width();
        image
                .reinterpret_stacked_2d_as_array(layers)
                .expect("asset should be 2d texture and height will always be evenly divisible with the given layers");
        image.texture_view_descriptor = Some(TextureViewDescriptor {
            dimension: Some(TextureViewDimension::Cube),
            ..default()
        });
    }

    cubemap.is_loaded = true;
}

// Step 3 when skybox texture is ready add skybox component to camera
fn add_skybox_to_camera(
    mut commands: Commands,
    cubemap: Res<Cubemap>,
    query: Query<Entity, (With<FreeCamera>, Without<Skybox>)>,
) {
    for entity_id in query.iter() {
        commands.entity(entity_id).insert(Skybox {
            // image: skybox_handle.clone(),
            image: Some(cubemap.image_handle.clone()),
            brightness: 1000.0,
            ..default()
        });
    }
}
