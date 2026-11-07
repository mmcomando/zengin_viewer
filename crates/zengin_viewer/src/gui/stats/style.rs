use bevy::color::Alpha;
use bevy::prelude::*;

use crate::zengin::MenuItemDefinition;

pub(super) struct StatsStyle;

impl StatsStyle {
    /// The status screen packs rows every 300 menu units, so its text is
    /// smaller than the main-menu items.
    pub(super) const FONT_SIZE: f32 = 12.0;
    pub(super) const MENU_AREA_FALLBACK: Color = Color::srgb(0.02, 0.02, 0.03);
    const TEXT_COLOR: Color = Color::srgb(0.85, 0.82, 0.72);

    pub(super) fn text_color(item: &MenuItemDefinition) -> Color {
        Self::TEXT_COLOR.with_alpha(f32::from(item.alpha) / 255.0)
    }
}
