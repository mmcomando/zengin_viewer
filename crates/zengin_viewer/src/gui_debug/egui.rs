use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass, egui};
use bevy_inspector_egui::DefaultInspectorConfigPlugin;

use crate::gameplay::camera::{CameraResource, EditorMode};
use crate::gui_debug::entity_details::{DetailsAction, entity_details_ui};
use crate::gui_debug::entity_list::{entity_list_ui, rebuild_rows};
use crate::gui_debug::info_tab::{InfoParams, info_tab_ui};
use crate::gui_debug::state::{
    DebugTab, DebugUiState, EntityQueryData, EntityQueryFilter, EntityRow, details_data,
};
use crate::perf_debug;
use crate::zengin::{ScriptResource, Selection};

pub struct GameDebugUI;
impl Plugin for GameDebugUI {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default());
        app.add_plugins(DefaultInspectorConfigPlugin);
        app.init_resource::<DebugUiState>();
        app.world_mut()
            .resource_mut::<EguiGlobalSettings>()
            .enable_absorb_bevy_input_system = true;
        app.add_systems(EguiPrimaryContextPass, debug_ui_system);
    }
}

struct DebugTabViewer<'a, 'w, 's> {
    info: &'a mut InfoParams<'w, 's>,
    rows: &'a [EntityRow],
    filter: &'a mut String,
    selected: &'a mut Option<Entity>,
    rows_dirty: &'a mut bool,
    right_open: &'a mut bool,
    game_selected: Option<Entity>,
    game_selected_name: &'a str,
}

impl egui_dock::TabViewer for DebugTabViewer<'_, '_, '_> {
    type Tab = DebugTab;

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        egui::Id::new(*tab)
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            DebugTab::Info => info_tab_ui(ui, self.info),
            DebugTab::Entities => {
                entity_list_ui(
                    ui,
                    self.rows,
                    self.filter,
                    self.selected,
                    self.rows_dirty,
                    self.right_open,
                    self.game_selected,
                    self.game_selected_name,
                );
            }
        }
    }

    fn is_closeable(&self, _tab: &Self::Tab) -> bool {
        false
    }

    fn allowed_in_windows(&self, _tab: &mut Self::Tab) -> bool {
        false
    }

    fn scroll_bars(&self, _tab: &Self::Tab) -> [bool; 2] {
        [false, false]
    }
}

#[allow(clippy::too_many_arguments)]
fn debug_ui_system(
    mut contexts: EguiContexts,
    mut state: ResMut<DebugUiState>,
    mut info: InfoParams,
    mut editor_mode: ResMut<EditorMode>,
    mut camera: ResMut<CameraResource>,
    scripts: Option<Res<ScriptResource>>,
    selection: Res<Selection>,
    type_registry: Res<AppTypeRegistry>,
    entities: Query<EntityQueryData, EntityQueryFilter>,
    camera_query: Query<&GlobalTransform, With<Camera3d>>,
    time: Res<Time>,
) -> Result {
    let _perf = perf_debug!();
    let refresh_due = state.refresh_timer.tick(time.delta()).just_finished();
    if state.left_open && (state.rows_dirty || refresh_due) {
        rebuild_rows(&entities, &mut state.rows);
        state.rows_dirty = false;
    }

    let ctx = contexts.ctx_mut()?;
    let mut root = egui::Ui::new(
        ctx.clone(),
        "debug_root".into(),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );

    let DebugUiState {
        left_open,
        dock,
        selected,
        right_open,
        filter,
        rows,
        rows_dirty,
        ..
    } = &mut *state;

    let mut open_requested = false;
    if *left_open {
        let mut close_requested = false;
        egui::Panel::left("debug_left_panel")
            .resizable(true)
            .default_size(360.0)
            .show(&mut root, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Collapse").clicked() {
                        close_requested = true;
                    }
                    ui.heading("Debug");
                });
                ui.separator();
                let mut viewer = DebugTabViewer {
                    info: &mut info,
                    rows: rows.as_slice(),
                    filter,
                    selected,
                    rows_dirty,
                    right_open,
                    game_selected: selection.target,
                    game_selected_name: &selection.name,
                };
                egui_dock::DockArea::new(dock)
                    .style(egui_dock::Style::from_egui(ui.style().as_ref()))
                    .show_close_buttons(false)
                    .show_leaf_close_all_buttons(false)
                    .show_leaf_collapse_buttons(false)
                    .tab_context_menus(false)
                    .draggable_tabs(false)
                    .show_add_buttons(false)
                    .show_inside(ui, &mut viewer);
            });
        if close_requested {
            *left_open = false;
        }
    } else {
        egui::Area::new(egui::Id::new("debug_toggle"))
            .anchor(egui::Align2::LEFT_TOP, [8.0, 8.0])
            .show(ctx, |ui| {
                if ui.button("Debug").clicked() {
                    open_requested = true;
                }
            });
    }

    if open_requested {
        *left_open = true;
        *rows_dirty = true;
        if !editor_mode.is_enabled() {
            *editor_mode = EditorMode::Enabled;
        }
        camera.set_free_camera();
    }

    let mut details_action = None;
    if let Some(entity) = *selected {
        egui::Panel::right("debug_details_panel")
            .resizable(true)
            .default_size(420.0)
            .show_collapsible(&mut root, right_open, |ui| {
                if let Ok(item) = entities.get(entity) {
                    let details = details_data(item);
                    details_action =
                        entity_details_ui(ui, &details, &type_registry.read(), scripts.as_deref());
                } else {
                    ui.heading("Entity gone");
                    ui.label(format!("Entity {entity} no longer exists."));
                    ui.horizontal(|ui| {
                        if ui.button("Collapse").clicked() {
                            details_action = Some(DetailsAction::Collapse);
                        }
                        if ui.button("Close").clicked() {
                            details_action = Some(DetailsAction::Close);
                        }
                    });
                }
            });
    }
    match details_action {
        Some(DetailsAction::Close) => *selected = None,
        Some(DetailsAction::Collapse) => *right_open = false,
        None => {}
    }
    if selected.is_some() && !*right_open {
        egui::Area::new(egui::Id::new("debug_details_toggle"))
            .anchor(egui::Align2::RIGHT_CENTER, [-8.0, 0.0])
            .show(ctx, |ui| {
                if ui.button("Details").clicked() {
                    *right_open = true;
                }
            });
    }

    if let Ok(camera) = camera_query.single() {
        camera_info_overlay(ctx, camera, selected.is_some() && *right_open);
    }
    Ok(())
}

/// Shows text with camera position and rotation.
fn camera_info_overlay(context: &egui::Context, camera: &GlobalTransform, right_panel_open: bool) {
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
    let anchor = if right_panel_open {
        egui::Align2::CENTER_BOTTOM
    } else {
        egui::Align2::RIGHT_TOP
    };
    egui::Area::new(egui::Id::new("camera_info"))
        .anchor(anchor, [-8.0, 8.0])
        .show(context, |ui| {
            let text = egui::RichText::new(text).monospace();
            ui.add(egui::Label::new(text).wrap_mode(egui::TextWrapMode::Extend));
        });
}
