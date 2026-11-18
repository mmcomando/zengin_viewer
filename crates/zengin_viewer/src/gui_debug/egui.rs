use bevy::prelude::*;
use bevy_egui::{
    EguiContexts, EguiPlugin, EguiPrimaryContextPass,
    egui::{self, Ui},
};

use crate::game_state::AppState;
use crate::gameplay::camera::{TELEPORT_LOCATIONS, TeleportToLocation};
use crate::gameplay::game_time::GameTime;
use crate::graphics::RenderOptions;
use crate::gui_debug::gizmos::GizmosOptions;
use crate::gui_debug::{
    RefreshStatistics,
    statistics::Statistics,
    toggle_visibility::{OptionsChanged, ShowOptions},
};
use crate::save::{QuickLoad, QuickSave};
use crate::zengin::{ImportantDialogOptions, RestartWorld, StatisticsZengin};

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
        "pos: {:6.1}, {:6.1}, {:6.1}\nrot: {:5.1}, {:5.1}, {:5.1}",
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
    mut refresh_statistics: MessageWriter<RefreshStatistics>,
    mut contexts: EguiContexts,
    mut toggle_info: ResMut<ShowOptions>,
    statistics: Res<Statistics>,
    statistics_zengin: Res<StatisticsZengin>,
    mut game_time: ResMut<GameTime>,
    mut render_options: ResMut<RenderOptions>,
    mut gizmos_options: ResMut<GizmosOptions>,
    mut important_dialog_options: ResMut<ImportantDialogOptions>,
    mut restart_world: MessageWriter<RestartWorld>,
    mut quick_save: MessageWriter<QuickSave>,
    mut quick_load: MessageWriter<QuickLoad>,
    mut teleports: MessageWriter<TeleportToLocation>,
    mut next_app_state: ResMut<NextState<AppState>>,
    camera_query: Query<&GlobalTransform, With<Camera3d>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    if let Ok(camera) = camera_query.single() {
        camera_info_overlay(ctx, camera);
    }

    egui::Window::new("Info")
        .default_open(false)
        .show(contexts.ctx_mut()?, |ui| {
            let mut options_changed = info_section(
                ui,
                &mut game_time,
                &mut important_dialog_options,
                &mut render_options,
            );
            statistics_section(ui, &statistics, &statistics_zengin, &mut refresh_statistics);
            options_changed |= visibility_section(ui, &mut toggle_info);
            gizmos_section(ui, &mut gizmos_options);
            teleport_section(ui, &mut teleports);
            world_section(
                ui,
                &mut restart_world,
                &mut quick_save,
                &mut quick_load,
                &mut next_app_state,
            );

            if options_changed {
                statistics_changed.write(OptionsChanged);
            }
        });
    Ok(())
}

/// Info grid with game time and render options.
///
/// Returns `true` when a render option that controls visibility changed.
fn info_section(
    ui: &mut Ui,
    game_time: &mut GameTime,
    important_dialog_options: &mut ImportantDialogOptions,
    render_options: &mut RenderOptions,
) -> bool {
    let mut changed = false;
    egui::Grid::new("Grid Info").num_columns(2).show(ui, |ui| {
        ui.label("Game Time");
        ui.add(
            egui::Slider::new(&mut game_time.total_hours, 0.0..=24.0)
                .custom_formatter(|hours, _| clock_string(hours as f32))
                .custom_parser(|text| parse_clock_string(text).map(f64::from)),
        );
        ui.end_row();

        ui.label("Time Speed");
        ui.add(egui::Slider::new(&mut game_time.speed, 0.0..=10.0).suffix("x"));
        ui.end_row();

        add_checkbox(
            ui,
            "Important Dialogs",
            &mut important_dialog_options.enabled,
            &mut false,
        );

        // Render options apply immediately, no message needed.
        add_checkbox(
            ui,
            "Sun Shadow Maps",
            &mut render_options.sun_shadow_maps,
            &mut changed,
        );
    });
    changed
}

/// Counters of loaded vs visible meshes, with a manual refresh button.
fn statistics_section(
    ui: &mut Ui,
    statistics: &Statistics,
    statistics_zengin: &StatisticsZengin,
    refresh_statistics: &mut MessageWriter<RefreshStatistics>,
) {
    egui::CollapsingHeader::new("Statistics")
        .default_open(true)
        .show(ui, |ui| {
            egui::Grid::new("Grid Statistics")
                .num_columns(2)
                .show(ui, |ui| {
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

                    ui.label("Refresh");
                    if ui.button("Refresh").clicked() {
                        refresh_statistics.write(RefreshStatistics);
                    }
                    ui.end_row();
                });
        });
}

/// Visibility toggles for world entities, collapsed by default.
///
/// Returns `true` when any toggle changed.
fn visibility_section(ui: &mut Ui, options: &mut ShowOptions) -> bool {
    let mut changed = false;
    egui::CollapsingHeader::new("Visibility")
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new("Grid Visibility")
                .num_columns(2)
                .show(ui, |ui| {
                    add_checkbox(ui, "World Mesh", &mut options.show_world_mesh, &mut changed);
                    add_checkbox(
                        ui,
                        "Static Meshes",
                        &mut options.show_static_meshes,
                        &mut changed,
                    );
                    add_checkbox(ui, "NPCs", &mut options.show_npcs, &mut changed);
                    add_checkbox(ui, "Chests", &mut options.show_chests, &mut changed);
                    add_checkbox(ui, "Mobs", &mut options.show_mobs, &mut changed);
                    add_checkbox(ui, "Items", &mut options.show_items, &mut changed);
                });
        });
    changed
}

/// Gizmo toggles, collapsed by default.
fn gizmos_section(ui: &mut Ui, options: &mut GizmosOptions) {
    let mut changed = false;
    egui::CollapsingHeader::new("Gizmos")
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new("Grid Gizmos")
                .num_columns(2)
                .show(ui, |ui| {
                    add_checkbox(
                        ui,
                        "Convex Colliders",
                        &mut options.convex_colliders,
                        &mut changed,
                    );
                    add_checkbox(
                        ui,
                        "Cuboid Colliders",
                        &mut options.cuboid_colliders,
                        &mut changed,
                    );
                    add_checkbox(
                        ui,
                        "Sphere Colliders",
                        &mut options.sphere_colliders,
                        &mut changed,
                    );
                    add_checkbox(
                        ui,
                        "Trimesh Colliders",
                        &mut options.trimesh_colliders,
                        &mut changed,
                    );
                    add_checkbox(
                        ui,
                        "Characters Colliders",
                        &mut options.characters_colliders,
                        &mut changed,
                    );
                    add_checkbox(ui, "Bones", &mut options.bones, &mut changed);
                    add_checkbox(ui, "Waypoints", &mut options.way_points, &mut changed);
                });
        });
}

fn teleport_section(ui: &mut Ui, teleports: &mut MessageWriter<TeleportToLocation>) {
    egui::CollapsingHeader::new("Teleport")
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for (index, location) in TELEPORT_LOCATIONS.iter().enumerate() {
                    if ui.button(location.name).clicked() {
                        teleports.write(TeleportToLocation(index));
                    }
                }
            });
        });
}

fn world_section(
    ui: &mut Ui,
    restart_world: &mut MessageWriter<RestartWorld>,
    quick_save: &mut MessageWriter<QuickSave>,
    quick_load: &mut MessageWriter<QuickLoad>,
    next_app_state: &mut NextState<AppState>,
) {
    egui::CollapsingHeader::new("World").show(ui, |ui| {
        ui.horizontal(|ui| {
            if ui.button("Restart World").clicked() {
                restart_world.write(RestartWorld);
            }
            if ui.button("Main Menu").clicked() {
                next_app_state.set(AppState::MainMenu);
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Quick Save").clicked() {
                quick_save.write(QuickSave);
            }
            if ui.button("Quick Load").clicked() {
                quick_load.write(QuickLoad);
            }
        });
    });
}
