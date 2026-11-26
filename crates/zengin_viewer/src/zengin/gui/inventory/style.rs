use bevy::prelude::*;

/// Visual style of the inventory UI, single place to tweak the look
pub(super) struct InventoryStyle;

impl InventoryStyle {
    pub(super) const COLUMNS: usize = 5;
    pub(super) const ROWS: usize = 8;
    pub(super) const SLOT_SIZE: f32 = 90.0;
    pub(super) const SLOT_GAP: f32 = 4.0;
    pub(super) const PANEL_PADDING: f32 = 12.0;
    pub(super) const SLOT_FONT_SIZE: f32 = 18.0;
    pub(super) const COUNT_FONT_SIZE: f32 = 15.0;
    pub(super) const HEADER_FONT_SIZE: f32 = 24.0;
    pub(super) const INFO_FONT_SIZE: f32 = 24.0;
    pub(super) const SLOT_NAME_CHARS: usize = 7;
    pub(super) const PANEL_BACKGROUND: Color = Color::srgba(0.06, 0.05, 0.04, 0.85);
    /// Translucent red fill drawn over equipped item slots.
    pub(super) const EQUIPPED_OVERLAY: Color = Color::srgba(0.7, 0.05, 0.05, 0.1);
    pub(super) const TEXT_COLOR: Color = Color::srgb(0.9, 0.8, 0.6);
    pub(super) const COUNT_TEXT_COLOR: Color = Color::srgb(0.95, 0.95, 0.9);
}
