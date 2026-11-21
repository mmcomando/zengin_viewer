use bevy::prelude::*;
use bevy_egui::egui;

use crate::gui_debug::state::{EntityQueryData, EntityQueryFilter, EntityRow, details_data};

/// Rebuilds the cached entity browser rows from the current world.
pub(crate) fn rebuild_rows(
    entities: &Query<EntityQueryData, EntityQueryFilter>,
    rows: &mut Vec<EntityRow>,
) {
    rows.clear();
    for item in entities.iter() {
        let details = details_data(item);
        let Some(kind) = details.kind() else {
            continue;
        };
        let name = details.name();
        let mut search = format!("{} {} {name}", kind.label(), kind.category());
        if let Some(base) = details.mob_base {
            search.push(' ');
            search.push_str(&base.focus_name);
        }
        let search = search.to_lowercase();
        rows.push(EntityRow {
            entity: details.entity,
            kind,
            name,
            search,
        });
    }
    rows.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.name.cmp(&right.name))
    });
}

/// Draws the entity browser tab.
pub(crate) fn entity_list_ui(
    ui: &mut egui::Ui,
    rows: &[EntityRow],
    filter: &mut String,
    selected: &mut Option<Entity>,
    rows_dirty: &mut bool,
    right_open: &mut bool,
    game_selected: Option<Entity>,
    game_selected_name: &str,
) {
    ui.horizontal(|ui| {
        ui.label("Filter");
        let width = ui.available_width();
        ui.add(egui::TextEdit::singleline(filter).desired_width(width));
    });
    ui.horizontal(|ui| {
        if ui.button("Clear").clicked() {
            filter.clear();
        }
        let response = ui.add_enabled(
            game_selected.is_some(),
            egui::Button::new("Use game selection"),
        );
        if response.clicked() {
            *selected = game_selected;
            *right_open = true;
        }
        if let Some(entity) = game_selected {
            response.on_hover_text(format!("{game_selected_name} ({entity})"));
        }
        if ui.button("Refresh").clicked() {
            *rows_dirty = true;
        }
    });

    let needle = filter.trim().to_lowercase();
    let visible: Vec<&EntityRow> = rows
        .iter()
        .filter(|row| needle.is_empty() || row.search.contains(&needle))
        .collect();

    ui.label(format!("{} entities", visible.len()));

    let row_height = ui.spacing().interact_size.y;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(ui, row_height, visible.len(), |ui, range| {
            for row in &visible[range] {
                let is_selected = *selected == Some(row.entity);
                let text = format!("[{}] {}", row.kind.label(), row.name);
                if ui.selectable_label(is_selected, text).clicked() {
                    *selected = Some(row.entity);
                    *right_open = true;
                }
            }
        });
}
