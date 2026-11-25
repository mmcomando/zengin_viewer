use bevy::ecs::system::{Local, SystemParam};
use bevy::prelude::*;
use bevy_egui::egui::{self, Ui};

use crate::debug::ReloadWorldEntities;
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
use crate::zengin::{DebugNpc, NpcAutoTalkOptions, RestartWorld, StatisticsZengin};

/// Resources and message writers used by the generic options tab.
#[derive(SystemParam)]
pub(crate) struct InfoParams<'w, 's> {
    game_time: ResMut<'w, GameTime>,
    auto_talk_options: ResMut<'w, NpcAutoTalkOptions>,
    render_options: ResMut<'w, RenderOptions>,
    statistics: Res<'w, Statistics>,
    statistics_zengin: Res<'w, StatisticsZengin>,
    show_options: ResMut<'w, ShowOptions>,
    gizmos_options: ResMut<'w, GizmosOptions>,
    statistics_changed: MessageWriter<'w, OptionsChanged>,
    refresh_statistics: MessageWriter<'w, RefreshStatistics>,
    restart_world: MessageWriter<'w, RestartWorld>,
    quick_save: MessageWriter<'w, QuickSave>,
    quick_load: MessageWriter<'w, QuickLoad>,
    teleports: MessageWriter<'w, TeleportToLocation>,
    reload_world_entities: MessageWriter<'w, ReloadWorldEntities>,
    next_app_state: ResMut<'w, NextState<AppState>>,
    debug_npc: ResMut<'w, DebugNpc>,
    debug_npc_text: Local<'s, String>,
}

pub(crate) fn info_tab_ui(ui: &mut Ui, info: &mut InfoParams) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut options_changed = info_section(
                ui,
                &mut info.game_time,
                &mut info.auto_talk_options,
                &mut info.render_options,
            );
            statistics_section(
                ui,
                &info.statistics,
                &info.statistics_zengin,
                &mut info.refresh_statistics,
            );
            options_changed |= visibility_section(ui, &mut info.show_options);
            gizmos_section(ui, &mut info.gizmos_options);
            debug_npc_section(ui, &mut info.debug_npc, &mut info.debug_npc_text);
            teleport_section(ui, &mut info.teleports);
            world_section(
                ui,
                &mut info.restart_world,
                &mut info.quick_save,
                &mut info.quick_load,
                &mut info.reload_world_entities,
                &mut info.next_app_state,
            );

            if options_changed {
                info.statistics_changed.write(OptionsChanged);
            }
        });
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

/// Info grid with game time and render options.
///
/// Returns `true` when a render option that controls visibility changed.
fn info_section(
    ui: &mut Ui,
    game_time: &mut GameTime,
    auto_talk_options: &mut NpcAutoTalkOptions,
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
            "Npc Auto Talk",
            &mut auto_talk_options.enabled,
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

fn debug_npc_section(ui: &mut Ui, debug_npc: &mut DebugNpc, text: &mut String) {
    egui::CollapsingHeader::new("Debug NPC Trace")
        .default_open(false)
        .show(ui, |ui| {
            // Pick up filters set elsewhere, e.g. replay commands.
            let current = debug_npc.text();
            if text.trim().to_lowercase() != current {
                text.clear();
                text.push_str(&current);
            }
            ui.horizontal(|ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(text)
                        .hint_text("npc symbol substring, comma separated"),
                );
                if response.changed() {
                    debug_npc.set(text);
                }
                if ui.button("Clear").clicked() {
                    text.clear();
                    debug_npc.set(text);
                }
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
    reload_world_entities: &mut MessageWriter<ReloadWorldEntities>,
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
        if ui
            .button("Reload Entities (10 frames)")
            .on_hover_text(
                "Serialize saveable objects, despawn world entities, then spawn the objects back",
            )
            .clicked()
        {
            reload_world_entities.write(ReloadWorldEntities);
        }
    });
}
