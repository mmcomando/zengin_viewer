use bevy::prelude::*;
use bevy_egui::{
    EguiContexts, EguiPlugin, EguiPrimaryContextPass,
    egui::{self, Ui},
};

use crate::gui::{
    statistics::Statistics,
    toggle_visibility::{OptionsChanged, ShowOptions},
};

pub struct GameDebugUI;
impl Plugin for GameDebugUI {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default());
        app.add_systems(EguiPrimaryContextPass, ui_example_system);
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

fn ui_example_system(
    mut statistics_changed: MessageWriter<OptionsChanged>,
    mut contexts: EguiContexts,
    mut toggle_info: ResMut<ShowOptions>,
    statistics: Res<Statistics>,
) -> Result {
    egui::Window::new("Info").show(contexts.ctx_mut()?, |ui| {
        let mut visibility_settings_changed = false;

        egui::Grid::new("Grid Info").num_columns(2).show(ui, |ui| {
            add_visible_elements(
                ui,
                "Zengin Models",
                statistics.zengin_models,
                statistics.zengin_models_visible,
            );
            add_visible_elements(
                ui,
                "Bevy Meshes",
                statistics.meshes,
                statistics.meshes_visible,
            );
            add_visible_elements(ui, "NPCs", statistics.npcs, statistics.npcs_visible);

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
        });

        if visibility_settings_changed {
            statistics_changed.write(OptionsChanged::default());
        }
    });
    Ok(())
}
