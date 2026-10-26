use bevy::prelude::*;

use crate::gameplay::game_time::GameTime;
use crate::ui_kit::{
    UiText, follow_selected_row, set_display, set_font_size, set_text, set_text_color,
    set_visibility,
};

use super::spawn::{
    JournalCategoryBoxMarker, JournalContainerMarker, JournalDetailScrollMarker, JournalListMarker,
    JournalTextMarker,
};
use super::state::{JournalCategory, JournalFocus, JournalUiState};
use super::style::JournalStyle;

pub(super) fn sync_journal_ui(
    mut state: ResMut<JournalUiState>,
    game_time: Res<GameTime>,
    mut commands: Commands,
    mut containers: Query<(&JournalContainerMarker, &mut Visibility)>,
    mut category_boxes: Query<(&JournalCategoryBoxMarker, &mut BorderColor)>,
    mut texts: Query<(
        &JournalTextMarker,
        &mut Text,
        &mut TextColor,
        &mut TextFont,
        &mut Node,
    )>,
    list: Single<
        (Entity, &mut ScrollPosition, &ComputedNode),
        (With<JournalListMarker>, Without<JournalDetailScrollMarker>),
    >,
    detail_scroll: Single<
        (&mut ScrollPosition, &ComputedNode),
        (With<JournalDetailScrollMarker>, Without<JournalListMarker>),
    >,
) {
    for (container, mut visibility) in containers.iter_mut() {
        let visible = match container {
            JournalContainerMarker::Root => state.open,
            JournalContainerMarker::Main => state.open && !state.detail_open,
            JournalContainerMarker::Detail => state.open && state.detail_open,
        };
        set_visibility(&mut visibility, visible);
    }
    if !state.open {
        return;
    }

    let detail_open = state.detail_open;
    let focus_categories = state.focus == JournalFocus::Categories && !detail_open;
    let (list_entity, mut list_scroll, list_computed) = list.into_inner();

    if state.rows_dirty {
        state.rows_dirty = false;
        spawn_missing_rows(&mut commands, list_entity, &state, &texts);
    }

    if state.reset_scroll {
        state.reset_scroll = false;
        if list_scroll.0 != Vec2::ZERO {
            list_scroll.0 = Vec2::ZERO;
        }
    }
    if state.focus == JournalFocus::Topics && !state.active.is_empty() {
        follow_selected_row(
            &mut list_scroll,
            list_computed,
            state.selected,
            JournalStyle::ROW_HEIGHT,
        );
    }

    let detail_text = if state.detail_dirty {
        state.detail_dirty = false;
        Some(state.detail_text())
    } else {
        None
    };
    let detail_footer = detail_open.then(|| sync_detail_scroll(&mut state, detail_scroll));
    let day_time = day_time_text(&game_time);

    for (marker, mut text, mut color, mut font, mut node) in texts.iter_mut() {
        match marker {
            JournalTextMarker::Category(category) => {
                let (desired_color, _) = category_look(*category, state.category, focus_categories);
                set_text_color(&mut color, desired_color);
            }
            JournalTextMarker::DayTime => set_text(&mut text, &day_time),
            JournalTextMarker::Row(index) => {
                let visible = *index < state.active.len();
                set_display(&mut node, visible);
                if !visible {
                    continue;
                }
                let Some(topic) = state
                    .active
                    .get(*index)
                    .and_then(|topic_index| state.topics.get(*topic_index))
                else {
                    continue;
                };
                let selected =
                    !detail_open && state.focus == JournalFocus::Topics && *index == state.selected;
                set_text(&mut text, &topic.name);
                set_text_color(
                    &mut color,
                    if selected {
                        JournalStyle::TOPIC_COLOR_SELECTED
                    } else {
                        JournalStyle::TOPIC_COLOR
                    },
                );
                set_font_size(
                    &mut font,
                    if selected {
                        JournalStyle::TOPIC_FONT_SIZE_SELECTED
                    } else {
                        JournalStyle::TOPIC_FONT_SIZE
                    },
                );
            }
            JournalTextMarker::Detail => {
                if let Some(detail_text) = &detail_text {
                    set_text(&mut text, detail_text);
                }
            }
            JournalTextMarker::DetailFooter => {
                if let Some(footer) = &detail_footer {
                    set_text(&mut text, footer);
                }
            }
        }
    }

    for (category_box, mut border) in category_boxes.iter_mut() {
        let (_, desired_border) = category_look(category_box.0, state.category, focus_categories);
        if border.top != desired_border {
            border.set_all(desired_border);
        }
    }
}

/// Spawns the rows that do not exist yet. New entities become visible to the
/// text query on the next frame, their content is set then.
fn spawn_missing_rows(
    commands: &mut Commands,
    list: Entity,
    state: &JournalUiState,
    texts: &Query<(
        &JournalTextMarker,
        &mut Text,
        &mut TextColor,
        &mut TextFont,
        &mut Node,
    )>,
) {
    let mut existing: Vec<usize> = texts
        .iter()
        .filter_map(|(marker, ..)| match marker {
            JournalTextMarker::Row(index) => Some(*index),
            _ => None,
        })
        .collect();
    existing.sort_unstable();
    for index in 0..state.active.len() {
        if existing.binary_search(&index).is_ok() {
            continue;
        }
        commands.spawn((
            ChildOf(list),
            JournalTextMarker::Row(index),
            Node {
                display: Display::None,
                width: Val::Percent(100.0),
                height: Val::Px(JournalStyle::ROW_HEIGHT),
                overflow: Overflow::clip(),
                ..default()
            },
            UiText::new(
                String::new(),
                JournalStyle::TOPIC_FONT_SIZE,
                JournalStyle::TOPIC_COLOR,
            ),
        ));
    }
}

/// Applies the requested line scroll to the entry viewer and returns the
/// page/percentage footer, clamped to the content height.
fn sync_detail_scroll(
    state: &mut JournalUiState,
    scroll: Single<
        (&mut ScrollPosition, &ComputedNode),
        (With<JournalDetailScrollMarker>, Without<JournalListMarker>),
    >,
) -> String {
    let (mut scroll, computed) = scroll.into_inner();
    let line_height = JournalStyle::DETAIL_LINE_HEIGHT;
    let viewport_height = computed.size().y;
    let content_height = computed.content_size().y;
    let max_scroll = (content_height - viewport_height).max(0.0);
    let max_lines = (max_scroll / line_height).ceil() as usize;
    if state.detail_scroll > max_lines {
        state.detail_scroll = max_lines;
    }
    let y = (state.detail_scroll as f32 * line_height).min(max_scroll);
    if (scroll.0.y - y).abs() > f32::EPSILON {
        scroll.0.y = y;
    }

    let visible_lines = (viewport_height / line_height).max(1.0);
    let total_lines = (content_height / line_height).ceil().max(1.0);
    let page_count = (total_lines / visible_lines).ceil().max(1.0) as u32;
    let page = ((y / viewport_height.max(1.0)).floor() as u32 + 1).min(page_count);
    let percent = if max_scroll > 0.0 {
        (y / max_scroll * 100.0).round() as u32
    } else {
        100
    };
    format!("{page}/{page_count} ({percent}%)")
}

fn category_look(
    category: JournalCategory,
    selected: JournalCategory,
    focused: bool,
) -> (Color, Color) {
    if category != selected {
        return (JournalStyle::LABEL_COLOR, JournalStyle::BORDER_COLOR);
    }
    let color = if focused {
        JournalStyle::LABEL_COLOR_FOCUSED
    } else {
        JournalStyle::LABEL_COLOR_SELECTED
    };
    (color, JournalStyle::BORDER_COLOR_SELECTED)
}

fn day_time_text(game_time: &GameTime) -> String {
    let hour = game_time.hour();
    let minute = game_time.minutes_of_day() % 60;
    format!("{}.Dzień:\nCzas: {hour:02}:{minute:02}", game_time.day)
}
