use bevy::prelude::*;

pub(super) struct MainMenuStyle;

impl MainMenuStyle {
    pub(super) const BACKGROUND: Color = Color::srgb(0.0, 0.0, 0.0);
    pub(super) const MENU_AREA_FALLBACK: Color = Color::srgb(0.02, 0.02, 0.03);
    pub(super) const TEXT_COLOR: Color = Color::srgb(0.78, 0.76, 0.68);
    pub(super) const TEXT_COLOR_SELECTED: Color = Color::srgb(1.0, 0.94, 0.8);
    pub(super) const TEXT_COLOR_DISABLED: Color = Color::srgb(0.42, 0.4, 0.36);
    pub(super) const ITEM_FONT_SIZE: f32 = 24.0;
    pub(super) const INFO_FONT_SIZE: f32 = 20.0;
    pub(super) const INFO_BOTTOM_OFFSET: f32 = 7.0;
}
