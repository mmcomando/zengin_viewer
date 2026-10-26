//! Journal (quest log) UI: category column, topic list and the entry viewer.
//!
//! The topic snapshot is taken from the script when the journal opens; the
//! journal only changes while a dialog runs and dialogs cannot run with the
//! journal open.

use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::gameplay::game_time::GameTime;
use crate::gui::InventoryUiState;
use crate::input::{GuiAction, GuiInputMarker, SharedAction, SharedInputMarker};
use crate::ui_kit::{
    UiText, follow_selected_row, set_display, set_font_size, set_text, set_text_color,
    set_visibility, spawn_gui_root,
};
use crate::zengin::{DialogUiState, JournalSection, JournalStatus, JournalTopic, ScriptResource};

pub struct JournalUiPlugin;

impl Plugin for JournalUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<JournalUiState>();
        app.add_systems(Startup, spawn_journal_ui);
        app.add_systems(
            Update,
            (toggle_journal, handle_journal_input, sync_journal_ui).chain(),
        );
    }
}

/// Visual style of the journal UI, single place to tweak the look
struct JournalStyle;

impl JournalStyle {
    const PANEL_BACKGROUND: Color = Color::srgba(0.0, 0.0, 0.0, 0.92);
    const BORDER_COLOR: Color = Color::srgb(0.72, 0.42, 0.05);
    const BORDER_COLOR_SELECTED: Color = Color::srgb(1.0, 0.65, 0.1);
    const BORDER_WIDTH: f32 = 2.0;
    const LABEL_COLOR: Color = Color::srgb(0.62, 0.6, 0.55);
    const LABEL_COLOR_SELECTED: Color = Color::srgb(0.95, 0.95, 0.92);
    const LABEL_COLOR_FOCUSED: Color = Color::srgb(0.95, 0.55, 0.08);
    const TOPIC_COLOR: Color = Color::srgb(0.1, 0.8, 0.18);
    const TOPIC_COLOR_SELECTED: Color = Color::srgb(0.95, 0.55, 0.08);
    const DETAIL_COLOR: Color = Color::srgb(0.9, 0.88, 0.82);
    const FOOTER_COLOR: Color = Color::srgb(0.55, 0.55, 0.52);
    const LABEL_FONT_SIZE: f32 = 24.0;
    const TOPIC_FONT_SIZE: f32 = 22.0;
    const TOPIC_FONT_SIZE_SELECTED: f32 = 24.0;
    const DETAIL_FONT_SIZE: f32 = 24.0;
    const FOOTER_FONT_SIZE: f32 = 20.0;
    const ROW_HEIGHT: f32 = 32.0;
    const DETAIL_LINE_HEIGHT: f32 = 30.0;
    const CATEGORY_HEIGHT: f32 = 84.0;
}

/// Sections of the journal, matching the original category widgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum JournalCategory {
    /// `CURRENTMISSIONS`.
    #[default]
    Current,
    /// `OLDMISSIONS`.
    Completed,
    /// `FAILEDMISSIONS`.
    Failed,
    /// `LOG` — notes.
    General,
}

impl JournalCategory {
    const ALL: [Self; 4] = [Self::Current, Self::Completed, Self::Failed, Self::General];

    /// Original menu labels in the Polish game data; they move into the menu
    /// script once menu data is loaded.
    fn label(self) -> &'static str {
        match self {
            Self::Current => "Obecne zadania",
            Self::Completed => "Wykonane zadania",
            Self::Failed => "Popsute zadania",
            Self::General => "Informacje ogólne",
        }
    }

    fn matches(self, topic: &JournalTopic) -> bool {
        match self {
            Self::Current => {
                topic.section == JournalSection::Mission && topic.status == JournalStatus::Running
            }
            Self::Completed => {
                topic.section == JournalSection::Mission
                    && matches!(
                        topic.status,
                        JournalStatus::Success | JournalStatus::Obsolete
                    )
            }
            Self::Failed => {
                topic.section == JournalSection::Mission && topic.status == JournalStatus::Failed
            }
            Self::General => topic.section == JournalSection::Note,
        }
    }
}

/// Which column receives the navigation input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum JournalFocus {
    #[default]
    Categories,
    Topics,
}

/// State of the journal UI: closed by default, topics snapshot on open.
#[derive(Resource, Default)]
pub struct JournalUiState {
    open: bool,
    category: JournalCategory,
    focus: JournalFocus,
    /// Selected entry in the filtered topic list.
    selected: usize,
    /// Indices into `topics` matching `category`, newest first.
    active: Vec<usize>,
    topics: Vec<JournalTopic>,
    detail_open: bool,
    /// Scroll offset of the entry viewer in text lines.
    detail_scroll: usize,
    detail_dirty: bool,
    rows_dirty: bool,
    reset_scroll: bool,
}

impl JournalUiState {
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    pub(crate) fn close(&mut self) {
        *self = Self::default();
    }

    /// Steps back from the entry viewer instead of closing the journal.
    /// Returns `false` when the journal itself was closed.
    pub(crate) fn escape(&mut self) -> bool {
        if self.detail_open {
            self.detail_open = false;
            self.detail_scroll = 0;
            return true;
        }
        self.close();
        false
    }

    fn open(&mut self, topics: Vec<JournalTopic>) {
        *self = Self {
            open: true,
            topics,
            rows_dirty: true,
            reset_scroll: true,
            ..default()
        };
        self.rebuild_active();
    }

    fn select_category(&mut self, category: JournalCategory) {
        if self.category == category {
            return;
        }
        self.category = category;
        self.rebuild_active();
        self.rows_dirty = true;
        self.reset_scroll = true;
    }

    fn move_category(&mut self, delta: isize) {
        let index = JournalCategory::ALL
            .iter()
            .position(|category| *category == self.category)
            .unwrap_or(0);
        let next = (index as isize + delta).clamp(0, JournalCategory::ALL.len() as isize - 1);
        self.select_category(JournalCategory::ALL[next as usize]);
    }

    fn rebuild_active(&mut self) {
        self.active.clear();
        for (index, topic) in self.topics.iter().enumerate().rev() {
            if self.category.matches(topic) {
                self.active.push(index);
            }
        }
        self.selected = 0;
    }

    fn current_topic(&self) -> Option<&JournalTopic> {
        let index = *self.active.get(self.selected)?;
        self.topics.get(index)
    }

    fn detail_text(&self) -> String {
        let Some(topic) = self.current_topic() else {
            return String::new();
        };
        topic.entries.join("\n---\n")
    }
}

/// Identifies the persistent journal containers, toggled via [`Visibility`]
#[derive(Component)]
enum JournalContainerMarker {
    Root,
    Main,
    Detail,
}

#[derive(Component)]
struct JournalCategoryBoxMarker(JournalCategory);

#[derive(Component)]
struct JournalListMarker;

#[derive(Component)]
struct JournalDetailScrollMarker;

/// Text node kind, so one query can update all journal texts without aliasing
#[derive(Component)]
enum JournalTextMarker {
    Category(JournalCategory),
    DayTime,
    Row(usize),
    Detail,
    DetailFooter,
}

/// Spawns the whole journal UI once at startup, hidden until opened.
fn spawn_journal_ui(mut commands: Commands) {
    let root = spawn_gui_root(&mut commands, JournalContainerMarker::Root);
    commands.entity(root).insert(GlobalZIndex(10));
    spawn_main_panel(&mut commands, root);
    spawn_detail_panel(&mut commands, root);
}

fn spawn_main_panel(commands: &mut Commands, root: Entity) {
    let panel = commands
        .spawn((
            ChildOf(root),
            JournalContainerMarker::Main,
            Visibility::Inherited,
            BackgroundColor(JournalStyle::PANEL_BACKGROUND),
            BorderColor::all(JournalStyle::BORDER_COLOR),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(4.0),
                top: Val::Percent(8.0),
                width: Val::Percent(92.0),
                height: Val::Percent(84.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(JournalStyle::BORDER_WIDTH)),
                ..default()
            },
        ))
        .id();

    let left = commands
        .spawn((
            ChildOf(panel),
            Node {
                width: Val::Percent(26.0),
                height: Val::Percent(100.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },
        ))
        .id();
    for category in JournalCategory::ALL {
        spawn_category_box(commands, left, category);
    }
    spawn_day_time_box(commands, left);

    commands.spawn((
        ChildOf(panel),
        JournalListMarker,
        ScrollPosition::default(),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
        BorderColor::all(JournalStyle::BORDER_COLOR),
        Node {
            flex_grow: 1.0,
            height: Val::Percent(100.0),
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(16.0)),
            border: UiRect::all(Val::Px(JournalStyle::BORDER_WIDTH)),
            overflow: Overflow::scroll_y(),
            ..default()
        },
    ));
}

fn spawn_category_box(commands: &mut Commands, parent: Entity, category: JournalCategory) {
    let category_box = commands
        .spawn((
            ChildOf(parent),
            JournalCategoryBoxMarker(category),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
            BorderColor::all(JournalStyle::BORDER_COLOR),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(JournalStyle::CATEGORY_HEIGHT),
                display: Display::Flex,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(JournalStyle::BORDER_WIDTH)),
                ..default()
            },
        ))
        .id();
    commands.spawn((
        ChildOf(category_box),
        JournalTextMarker::Category(category),
        UiText::new(
            category.label(),
            JournalStyle::LABEL_FONT_SIZE,
            JournalStyle::LABEL_COLOR,
        )
        .justify(Justify::Center),
    ));
}

fn spawn_day_time_box(commands: &mut Commands, parent: Entity) {
    let box_entity = commands
        .spawn((
            ChildOf(parent),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
            BorderColor::all(JournalStyle::BORDER_COLOR),
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::FlexEnd,
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(JournalStyle::BORDER_WIDTH)),
                ..default()
            },
        ))
        .id();
    commands.spawn((
        ChildOf(box_entity),
        JournalTextMarker::DayTime,
        UiText::new(
            String::new(),
            JournalStyle::LABEL_FONT_SIZE,
            JournalStyle::LABEL_COLOR_SELECTED,
        )
        .justify(Justify::Right),
    ));
}

fn spawn_detail_panel(commands: &mut Commands, root: Entity) {
    let panel = commands
        .spawn((
            ChildOf(root),
            JournalContainerMarker::Detail,
            Visibility::Hidden,
            BackgroundColor(JournalStyle::PANEL_BACKGROUND),
            BorderColor::all(JournalStyle::BORDER_COLOR),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(4.0),
                top: Val::Percent(8.0),
                width: Val::Percent(92.0),
                height: Val::Percent(84.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(40.0)),
                border: UiRect::all(Val::Px(JournalStyle::BORDER_WIDTH)),
                ..default()
            },
        ))
        .id();

    let scroll = commands
        .spawn((
            ChildOf(panel),
            JournalDetailScrollMarker,
            ScrollPosition::default(),
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                ..default()
            },
        ))
        .id();
    commands.spawn((
        ChildOf(scroll),
        JournalTextMarker::Detail,
        Node {
            width: Val::Percent(100.0),
            ..default()
        },
        UiText::new(
            String::new(),
            JournalStyle::DETAIL_FONT_SIZE,
            JournalStyle::DETAIL_COLOR,
        ),
    ));
    commands.spawn((
        ChildOf(panel),
        JournalTextMarker::DetailFooter,
        Node {
            width: Val::Percent(100.0),
            margin: UiRect::top(Val::Px(12.0)),
            ..default()
        },
        UiText::new(
            String::new(),
            JournalStyle::FOOTER_FONT_SIZE,
            JournalStyle::FOOTER_COLOR,
        )
        .justify(Justify::Center),
    ));
}

/// The dialog UI and the inventory take precedence over the journal.
fn toggle_journal(
    action_state: Single<&ActionState<SharedAction>, With<SharedInputMarker>>,
    dialog_ui: Res<DialogUiState>,
    inventory_ui: Res<InventoryUiState>,
    mut state: ResMut<JournalUiState>,
    gothic_script: Res<ScriptResource>,
) {
    if !action_state.just_pressed(&SharedAction::Journal) {
        return;
    }
    if state.open {
        state.close();
        return;
    }
    if dialog_ui.is_open() || inventory_ui.is_open() {
        return;
    }

    let topics = gothic_script.lock().journal_topics();
    let count = topics.len();
    state.open(topics);
    info!("Journal opened with {count} topic(s)");
}

fn handle_journal_input(
    action_state: Single<&ActionState<GuiAction>, With<GuiInputMarker>>,
    mut state: ResMut<JournalUiState>,
) {
    if !state.open {
        return;
    }

    if state.detail_open {
        if action_state.just_pressed(&GuiAction::CancelAction) {
            state.detail_open = false;
            state.detail_scroll = 0;
            return;
        }
        if action_state.just_pressed(&GuiAction::Up) {
            state.detail_scroll = state.detail_scroll.saturating_sub(1);
        }
        if action_state.just_pressed(&GuiAction::Down) {
            state.detail_scroll += 1;
        }
        return;
    }

    if action_state.just_pressed(&GuiAction::CancelAction) {
        match state.focus {
            JournalFocus::Topics => state.focus = JournalFocus::Categories,
            JournalFocus::Categories => state.close(),
        }
        return;
    }

    match state.focus {
        JournalFocus::Categories => {
            if action_state.just_pressed(&GuiAction::Up) {
                state.move_category(-1);
            }
            if action_state.just_pressed(&GuiAction::Down) {
                state.move_category(1);
            }
            if action_state.just_pressed(&GuiAction::Right)
                || action_state.just_pressed(&GuiAction::Confirm)
            {
                state.focus = JournalFocus::Topics;
            }
        }
        JournalFocus::Topics => {
            if action_state.just_pressed(&GuiAction::Left) {
                state.focus = JournalFocus::Categories;
            }
            if action_state.just_pressed(&GuiAction::Up) {
                state.selected = state.selected.saturating_sub(1);
            }
            if action_state.just_pressed(&GuiAction::Down)
                && state.selected + 1 < state.active.len()
            {
                state.selected += 1;
            }
            if action_state.just_pressed(&GuiAction::Confirm) && !state.active.is_empty() {
                state.detail_open = true;
                state.detail_scroll = 0;
                state.detail_dirty = true;
            }
        }
    }
}

fn sync_journal_ui(
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
