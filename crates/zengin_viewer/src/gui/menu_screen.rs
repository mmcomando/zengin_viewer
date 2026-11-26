//! Shared layout and item painting of a MENU.DAT screen definition.

use bevy::color::Alpha;
use bevy::prelude::*;

use crate::zengin::{
    MENU_FLAG_ALIGN_CENTER, MENU_FLAG_DONT_SCALE_DIM, MENU_ITEM_FLAG_TXT_CENTER, MENU_SCRIPT_DIV,
    MenuDefinition, MenuItemDefinition, get_full_texture_path,
};
use dev_util::ui_kit::UiText;

/// Text entity spawned for one menu item, `slot` is the item index in the
/// screen's `items[]` array.
#[derive(Component)]
pub(crate) struct MenuScreenTextMarker {
    pub(crate) slot: usize,
}

/// Pixel rectangle of a menu screen inside the window.
pub(crate) struct MenuArea {
    pub(crate) left: f32,
    pub(crate) top: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

/// Item size in menu units the original layout uses for `dim == -1`.
const DEFAULT_ITEM_WIDTH: i32 = 8192;
const DEFAULT_ITEM_HEIGHT: i32 = 750;

/// Spawns the screen backpic and one node per item, returning the screen area
/// entity items were parented to.
pub(crate) fn spawn_menu_screen(
    commands: &mut Commands,
    asset_server: &AssetServer,
    definition: &MenuDefinition,
    parent: Entity,
    window_size: Vec2,
    scale: f32,
    font_size: f32,
    fallback: Color,
    text_of: impl Fn(&MenuItemDefinition) -> String,
    color_of: impl Fn(&MenuItemDefinition) -> Color,
) -> Entity {
    let area = menu_area(definition, window_size, scale);

    let mut area_commands = commands.spawn((
        ChildOf(parent),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(area.left),
            top: Val::Px(area.top),
            width: Val::Px(area.width),
            height: Val::Px(area.height),
            ..default()
        },
    ));
    if let Some(back_pic) = &definition.back_pic {
        let mut image = ImageNode::new(asset_server.load(get_full_texture_path(back_pic)));
        image.color = white_with_alpha(definition.alpha);
        image.image_mode = NodeImageMode::Stretch;
        area_commands.insert(image);
    } else {
        area_commands.insert(BackgroundColor(fallback));
    }
    let area_id = area_commands.id();

    for item in &definition.items {
        spawn_item(
            commands,
            asset_server,
            item,
            area_id,
            Vec2::new(area.width, area.height),
            font_size,
            &text_of,
            &color_of,
        );
    }
    area_id
}

pub(crate) fn menu_area(definition: &MenuDefinition, window_size: Vec2, scale: f32) -> MenuArea {
    let (width, height) = if definition.flags & MENU_FLAG_DONT_SCALE_DIM != 0 {
        (
            definition.dim.0 as f32 * 640.0 * scale / MENU_SCRIPT_DIV,
            definition.dim.1 as f32 * 480.0 * scale / MENU_SCRIPT_DIV,
        )
    } else {
        (
            definition.dim.0 as f32 * window_size.x / MENU_SCRIPT_DIV,
            definition.dim.1 as f32 * window_size.y / MENU_SCRIPT_DIV,
        )
    };
    let (left, top) = if definition.flags & MENU_FLAG_ALIGN_CENTER != 0 {
        (
            (window_size.x - width) / 2.0,
            (window_size.y - height) / 2.0,
        )
    } else {
        (
            definition.pos.0 as f32 * 640.0 * scale / MENU_SCRIPT_DIV,
            definition.pos.1 as f32 * 480.0 * scale / MENU_SCRIPT_DIV,
        )
    };
    MenuArea {
        left,
        top,
        width,
        height,
    }
}

/// Pixel rect of one item inside a menu area, `(x, y, width, height)`.
pub(crate) fn item_rect(item: &MenuItemDefinition, area_size: Vec2) -> (f32, f32, f32, f32) {
    let x = item.pos.0 as f32 * area_size.x / MENU_SCRIPT_DIV;
    let y = item.pos.1 as f32 * area_size.y / MENU_SCRIPT_DIV;
    let width = item_dimension(item.dim.0, DEFAULT_ITEM_WIDTH) * area_size.x / MENU_SCRIPT_DIV;
    let height = item_dimension(item.dim.1, DEFAULT_ITEM_HEIGHT) * area_size.y / MENU_SCRIPT_DIV;
    (x, y, width.max(0.0), height.max(0.0))
}

/// Text of a menu item: the script stores line breaks as a literal `\n`.
pub(crate) fn menu_text(value: &str) -> String {
    value.replace("\\n", "\n")
}

#[allow(
    clippy::too_many_arguments,
    reason = "Pure layout painter, every value is independent"
)]
fn spawn_item(
    commands: &mut Commands,
    asset_server: &AssetServer,
    item: &MenuItemDefinition,
    area: Entity,
    area_size: Vec2,
    font_size: f32,
    text_of: &impl Fn(&MenuItemDefinition) -> String,
    color_of: &impl Fn(&MenuItemDefinition) -> Color,
) {
    let (x, y, width, height) = item_rect(item, area_size);

    let centered = item.flags & MENU_ITEM_FLAG_TXT_CENTER != 0;
    let mut row = commands.spawn((
        ChildOf(area),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(x),
            top: Val::Px(y),
            width: Val::Px(width.max(0.0)),
            height: Val::Px(height.max(0.0)),
            display: Display::Flex,
            justify_content: if centered {
                JustifyContent::Center
            } else {
                JustifyContent::FlexStart
            },
            align_items: if centered {
                AlignItems::Center
            } else {
                AlignItems::FlexStart
            },
            ..default()
        },
    ));
    if let Some(back_pic) = &item.back_pic {
        let mut image = ImageNode::new(asset_server.load(get_full_texture_path(back_pic)));
        image.color = white_with_alpha(item.alpha);
        image.image_mode = NodeImageMode::Stretch;
        row.insert(image);
    }
    let text = menu_text(&text_of(item));

    let ui_text = UiText::new(text, font_size, color_of(item));
    let ui_text = if centered {
        ui_text.justify(Justify::Center)
    } else {
        ui_text
    };
    let row_id = row.id();
    commands.spawn((
        ChildOf(row_id),
        MenuScreenTextMarker { slot: item.slot },
        ui_text,
    ));
}

fn item_dimension(value: i32, default: i32) -> f32 {
    let value = if value == -1 { default } else { value };
    value.max(0) as f32
}

fn white_with_alpha(alpha: u8) -> Color {
    Color::WHITE.with_alpha(f32::from(alpha) / 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_dimension_uses_defaults_for_minus_one() {
        assert_eq!(item_dimension(-1, DEFAULT_ITEM_WIDTH), 8192.0);
        assert_eq!(item_dimension(-1, DEFAULT_ITEM_HEIGHT), 750.0);
    }

    #[test]
    fn item_dimension_keeps_positive_values() {
        assert_eq!(item_dimension(100, DEFAULT_ITEM_WIDTH), 100.0);
    }

    #[test]
    fn item_dimension_clamps_other_negatives() {
        assert_eq!(item_dimension(-2, DEFAULT_ITEM_WIDTH), 0.0);
    }
}
