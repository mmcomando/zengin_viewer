use bevy::prelude::*;
use bevy_egui::{
    EguiContexts, EguiPlugin, EguiPrimaryContextPass,
    egui::{self, Ui},
};

use crate::gameplay::game_time::GameTime;
use crate::graphics::render_options::RenderOptions;
use crate::gui_debug::colliders_gizmos::ShowConvexColliderGizmos;
use crate::gui_debug::{
    statistics::Statistics,
    toggle_visibility::{OptionsChanged, ShowOptions},
};
use crate::zengin::StatisticsZengin;

pub struct GameDebugUI;
impl Plugin for GameDebugUI {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default());
        app.add_systems(EguiPrimaryContextPass, ui_info_system);
    }
}

fn add_checkbox(ui: &mut Ui, label: &str, value: &mut bool, changed: &mut bool) {
    let mut value_tmp = *value;
    ui.label(label);
    ui.add(egui::Checkbox::new(&mut value_tmp, ""));
    ui.end_row();
    *changed |= *value != value_tmp;
    *value = value_tmp;
}

fn add_visible_elements(ui: &mut Ui, label: &str, value_all: i64, value_visible: i64) {
    let mut value_visible_mut = value_visible;
    let mut value_all_mut = value_all;
    ui.label(label);
    ui.add_enabled(false, egui::DragValue::new(&mut value_visible_mut));
    ui.add_enabled(false, egui::DragValue::new(&mut value_all_mut));
    ui.end_row();
}

fn hour_of(total_hours: f32) -> u32 {
    (total_hours.floor() as u32) % 24
}

fn minute_of(total_hours: f32) -> u32 {
    (total_hours.fract() * 60.0).floor() as u32
}

/// Formats hours in `[0, 24)` as a `HH:MM` clock string.
fn clock_string(total_hours: f32) -> String {
    format!("{:02}:{:02}", hour_of(total_hours), minute_of(total_hours))
}

/// Parses `HH:MM` (a missing part defaults to `0`) into hours in `[0, 24)`.
fn parse_clock_string(text: &str) -> Option<f32> {
    let (hour, minute) = match text.trim().split_once(':') {
        Some((hour, minute)) => (hour, minute),
        None => (text.trim(), "0"),
    };
    let hour: f32 = hour.trim().parse().ok()?;
    let minute: f32 = minute.trim().parse().ok()?;
    if !(0.0..=23.0).contains(&hour) || !(0.0..=59.0).contains(&minute) {
        return None;
    }
    Some(hour + minute / 60.0)
}

/// Shows text in the top right corner with camera position and rotation.
fn camera_info_overlay(context: &egui::Context, camera: &GlobalTransform) {
    let position = camera.translation();
    let (yaw, pitch, roll) = camera.rotation().to_euler(EulerRot::YXZ);
    let degrees = |rad: f32| rad.to_degrees();
    // Printed in axis labeled order (x = pitch, y = yaw, z = roll) to match
    // the rotation format of replay location commands.
    let text = format!(
        "pos: {:6.1}, {:6.1}, {:6.1}\nrot: {:5.1} {:5.1} {:5.1}",
        position.x,
        position.y,
        position.z,
        degrees(pitch),
        degrees(yaw),
        degrees(roll)
    );
    egui::Area::new(egui::Id::new("camera_info"))
        .anchor(egui::Align2::RIGHT_TOP, [-8.0, 8.0])
        .show(context, |ui| {
            let text = egui::RichText::new(text).monospace();
            ui.add(egui::Label::new(text).wrap_mode(egui::TextWrapMode::Extend));
        });
}

#[allow(clippy::too_many_arguments)]
fn ui_info_system(
    mut statistics_changed: MessageWriter<OptionsChanged>,
    mut contexts: EguiContexts,
    mut toggle_info: ResMut<ShowOptions>,
    statistics: Res<Statistics>,
    statistics_zengin: Res<StatisticsZengin>,
    mut game_time: ResMut<GameTime>,
    mut render_options: ResMut<RenderOptions>,
    mut convex_collider_gizmos: ResMut<ShowConvexColliderGizmos>,
    camera_query: Query<&GlobalTransform, With<Camera3d>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    if let Ok(camera) = camera_query.single() {
        camera_info_overlay(ctx, camera);
    }

    egui::Window::new("Info")
        .default_open(false)
        .show(contexts.ctx_mut()?, |ui| {
            let mut visibility_settings_changed = false;

            egui::Grid::new("Grid Info").num_columns(2).show(ui, |ui| {
                ui.label("Game Time");
                ui.add(
                    egui::Slider::new(&mut game_time.total_hours, 0.0..=24.0)
                        .custom_formatter(|hours, _| clock_string(hours as f32))
                        .custom_parser(|text| parse_clock_string(text).map(|hours| hours as f64)),
                );
                ui.end_row();

                ui.label("Time Speed");
                ui.add(egui::Slider::new(&mut game_time.speed, 0.0..=10.0).suffix("x"));
                ui.end_row();

                add_visible_elements(
                    ui,
                    "Zengin Models",
                    statistics_zengin.zengin_models,
                    statistics_zengin.zengin_models_visible,
                );
                add_visible_elements(
                    ui,
                    "Bevy Meshes",
                    statistics.meshes,
                    statistics.meshes_visible,
                );
                add_visible_elements(
                    ui,
                    "NPCs",
                    statistics_zengin.npcs,
                    statistics_zengin.npcs_visible,
                );

                add_checkbox(
                    ui,
                    "Show World Mesh",
                    &mut toggle_info.show_world_mesh,
                    &mut visibility_settings_changed,
                );

                add_checkbox(
                    ui,
                    "Show Static Meshes",
                    &mut toggle_info.show_static_meshes,
                    &mut visibility_settings_changed,
                );

                add_checkbox(
                    ui,
                    "Show NPCs",
                    &mut toggle_info.show_npcs,
                    &mut visibility_settings_changed,
                );
                add_checkbox(
                    ui,
                    "Show Gizmos",
                    &mut toggle_info.show_gizmos,
                    &mut visibility_settings_changed,
                );
                // Render options apply immediately, no message needed.
                add_checkbox(
                    ui,
                    "Sun Shadow Maps",
                    &mut render_options.sun_shadow_maps,
                    &mut visibility_settings_changed,
                );
                add_checkbox(
                    ui,
                    "Disco Shadow Maps",
                    &mut render_options.disco_shadow_maps,
                    &mut visibility_settings_changed,
                );
                // Render options apply immediately, no message needed.
                add_checkbox(
                    ui,
                    "Convex Collider Gizmos",
                    &mut convex_collider_gizmos.0,
                    &mut visibility_settings_changed,
                );
            });

            if visibility_settings_changed {
                statistics_changed.write(OptionsChanged);
            }
        });
    Ok(())
}
