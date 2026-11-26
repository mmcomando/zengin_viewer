use bevy::prelude::*;

pub struct MainMenuStyle;

impl MainMenuStyle {
    pub const BACKGROUND: Color = Color::srgb(0.0, 0.0, 0.0);
    pub const MENU_AREA_FALLBACK: Color = Color::srgb(0.02, 0.02, 0.03);
    pub const TEXT_COLOR: Color = Color::srgb(0.78, 0.76, 0.68);
    pub const TEXT_COLOR_SELECTED: Color = Color::srgb(1.0, 0.94, 0.8);
    pub const TEXT_COLOR_DISABLED: Color = Color::srgb(0.42, 0.4, 0.36);
    pub const ITEM_FONT_SIZE: f32 = 24.0;
    /// Load-screen slots and metadata values use the script's small font.
    pub const LOAD_FONT_SIZE: f32 = 13.0;
    /// Info bar under the load screen; the MENU.DAT slots are much shorter than
    /// the main-menu rows.
    pub const LOAD_INFO_FONT_SIZE: f32 = 13.0;
    pub const INFO_FONT_SIZE: f32 = 20.0;
    pub const INFO_BOTTOM_OFFSET: f32 = 7.0;
}
