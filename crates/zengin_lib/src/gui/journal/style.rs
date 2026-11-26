use bevy::prelude::*;

/// Visual style of the journal UI, single place to tweak the look.
pub struct JournalStyle;

impl JournalStyle {
    pub const TEXT_COLOR: Color = Color::srgb(0.85, 0.82, 0.72);
    pub const TEXT_COLOR_SELECTED: Color = Color::srgb(1.0, 0.72, 0.25);
    pub const CATEGORY_FONT_SIZE: f32 = 18.0;
    pub const DATETIME_FONT_SIZE: f32 = 14.0;
    pub const TOPIC_FONT_SIZE: f32 = 18.0;
    pub const DETAIL_FONT_SIZE: f32 = 20.0;
    pub const FOOTER_FONT_SIZE: f32 = 14.0;
    pub const ROW_HEIGHT: f32 = 24.0;
    pub const DETAIL_LINE_HEIGHT: f32 = 24.0;
    /// Entry viewer text inset as a fraction of the menu area size.
    pub const DETAIL_PADDING_X: f32 = 0.07;
    pub const DETAIL_PADDING_Y: f32 = 0.08;
}
