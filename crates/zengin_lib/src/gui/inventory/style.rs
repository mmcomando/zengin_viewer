use bevy::prelude::*;

/// Visual style of the inventory UI, single place to tweak the look
pub struct InventoryStyle;

impl InventoryStyle {
    pub const COLUMNS: usize = 5;
    pub const ROWS: usize = 8;
    pub const SLOT_SIZE: f32 = 90.0;
    pub const SLOT_GAP: f32 = 4.0;
    pub const PANEL_PADDING: f32 = 12.0;
    pub const SLOT_FONT_SIZE: f32 = 18.0;
    pub const COUNT_FONT_SIZE: f32 = 15.0;
    pub const HEADER_FONT_SIZE: f32 = 24.0;
    pub const INFO_FONT_SIZE: f32 = 24.0;
    pub const SLOT_NAME_CHARS: usize = 7;
    pub const PANEL_BACKGROUND: Color = Color::srgba(0.06, 0.05, 0.04, 0.85);
    /// Translucent red fill drawn over equipped item slots.
    pub const EQUIPPED_OVERLAY: Color = Color::srgba(0.7, 0.05, 0.05, 0.1);
    pub const TEXT_COLOR: Color = Color::srgb(0.9, 0.8, 0.6);
    pub const COUNT_TEXT_COLOR: Color = Color::srgb(0.95, 0.95, 0.9);
}
