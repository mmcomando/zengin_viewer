use bevy::prelude::*;

/// Visual style of the journal UI, single place to tweak the look
pub(super) struct JournalStyle;

impl JournalStyle {
    pub(super) const PANEL_BACKGROUND: Color = Color::srgba(0.0, 0.0, 0.0, 0.92);
    pub(super) const BORDER_COLOR: Color = Color::srgb(0.72, 0.42, 0.05);
    pub(super) const BORDER_COLOR_SELECTED: Color = Color::srgb(1.0, 0.65, 0.1);
    pub(super) const BORDER_WIDTH: f32 = 2.0;
    pub(super) const LABEL_COLOR: Color = Color::srgb(0.62, 0.6, 0.55);
    pub(super) const LABEL_COLOR_SELECTED: Color = Color::srgb(0.95, 0.95, 0.92);
    pub(super) const LABEL_COLOR_FOCUSED: Color = Color::srgb(0.95, 0.55, 0.08);
    pub(super) const TOPIC_COLOR: Color = Color::srgb(0.1, 0.8, 0.18);
    pub(super) const TOPIC_COLOR_SELECTED: Color = Color::srgb(0.95, 0.55, 0.08);
    pub(super) const DETAIL_COLOR: Color = Color::srgb(0.9, 0.88, 0.82);
    pub(super) const FOOTER_COLOR: Color = Color::srgb(0.55, 0.55, 0.52);
    pub(super) const LABEL_FONT_SIZE: f32 = 24.0;
    pub(super) const TOPIC_FONT_SIZE: f32 = 22.0;
    pub(super) const TOPIC_FONT_SIZE_SELECTED: f32 = 24.0;
    pub(super) const DETAIL_FONT_SIZE: f32 = 24.0;
    pub(super) const FOOTER_FONT_SIZE: f32 = 20.0;
    pub(super) const ROW_HEIGHT: f32 = 32.0;
    pub(super) const DETAIL_LINE_HEIGHT: f32 = 30.0;
    pub(super) const CATEGORY_HEIGHT: f32 = 84.0;
}
