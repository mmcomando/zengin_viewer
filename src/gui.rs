use bevy::{
    camera_controller::free_camera::{FreeCamera, FreeCameraState},
    prelude::*,
};

// Plugin that handles camera settings controls and information text
pub struct CameraSettingsPlugin;
impl Plugin for CameraSettingsPlugin {
    fn build(&self, app: &mut App) {
        if false {
            app.add_systems(PostStartup, spawn_text)
                // .add_systems(Update, update_cameras)
                .add_systems(Update, (update_camera_settings, update_text));
        }
    }
}

#[derive(Component)]
struct InfoText;

fn spawn_text(mut commands: Commands, free_camera_query: Query<&FreeCamera>) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: px(-16),
            left: px(12),
            ..default()
        },
        children![Text::new(format!(
            "{}",
            free_camera_query.single().unwrap()
        ))],
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            bottom: px(12),
            left: px(12),
            ..default()
        },
        children![Text::new(concat![
            "Z/X: decrease/increase sensitivity\n",
            "C/V: decrease/increase friction\n",
            "F/G: decrease/increase scroll factor\n",
            "B: enable/disable controller",
        ]),],
    ));

    // Mutable text marked with component
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            right: px(12),
            ..default()
        },
        children![(InfoText, Text::new(""))],
    ));
}

// fn update_cameras(camera_query: Query<(Entity, &mut Camera3d)>) {
//     for (entity_id, _camera) in camera_query.iter() {
//         println!("Entity({:?})", entity_id);
//     }
//     // let (entity, mut camera) = camera_query.single_mut().unwrap();
// }

fn update_camera_settings(
    mut camera_query: Query<(&mut FreeCamera, &mut FreeCameraState)>,
    input: Res<ButtonInput<KeyCode>>,
) {
    let (mut free_camera, mut free_camera_state) = camera_query.single_mut().unwrap();

    if input.pressed(KeyCode::KeyZ) {
        free_camera.sensitivity = (free_camera.sensitivity - 0.005).max(0.005);
    }
    if input.pressed(KeyCode::KeyX) {
        free_camera.sensitivity += 0.005;
    }
    if input.pressed(KeyCode::KeyC) {
        free_camera.friction = (free_camera.friction - 0.2).max(0.0);
    }
    if input.pressed(KeyCode::KeyV) {
        free_camera.friction += 0.2;
    }
    if input.pressed(KeyCode::KeyF) {
        free_camera.scroll_factor = (free_camera.scroll_factor - 0.02).max(0.02);
    }
    if input.pressed(KeyCode::KeyG) {
        free_camera.scroll_factor += 0.02;
    }
    if input.just_pressed(KeyCode::KeyB) {
        free_camera_state.enabled = !free_camera_state.enabled;
    }
}

fn update_text(
    mut text_query: Query<&mut Text, With<InfoText>>,
    camera_query: Query<(&FreeCamera, &FreeCameraState)>,
) {
    let mut text = text_query.single_mut().unwrap();

    let (free_camera, free_camera_state) = camera_query.single().unwrap();

    text.0 = format!(
        "Enabled: {},\nSensitivity: {:.03}\nFriction: {:.01}\nScroll factor: {:.02}\nWalk Speed: {:.02}\nRun Speed: {:.02}\nSpeed: {:.02}",
        free_camera_state.enabled,
        free_camera.sensitivity,
        free_camera.friction,
        free_camera.scroll_factor,
        free_camera.walk_speed,
        free_camera.run_speed,
        free_camera_state.velocity.length(),
    );
}
