use bevy::prelude::*;

/// Visual style of the journal UI, single place to tweak the look.
pub(super) struct JournalStyle;

impl JournalStyle {
    pub(super) const TEXT_COLOR: Color = Color::srgb(0.85, 0.82, 0.72);
    pub(super) const TEXT_COLOR_SELECTED: Color = Color::srgb(1.0, 0.72, 0.25);
    pub(super) const CATEGORY_FONT_SIZE: f32 = 18.0;
    pub(super) const DATETIME_FONT_SIZE: f32 = 14.0;
    pub(super) const TOPIC_FONT_SIZE: f32 = 18.0;
    pub(super) const DETAIL_FONT_SIZE: f32 = 20.0;
    pub(super) const FOOTER_FONT_SIZE: f32 = 14.0;
    pub(super) const ROW_HEIGHT: f32 = 24.0;
    pub(super) const DETAIL_LINE_HEIGHT: f32 = 24.0;
    /// Entry viewer text inset as a fraction of the menu area size.
    pub(super) const DETAIL_PADDING_X: f32 = 0.07;
    pub(super) const DETAIL_PADDING_Y: f32 = 0.08;
}
