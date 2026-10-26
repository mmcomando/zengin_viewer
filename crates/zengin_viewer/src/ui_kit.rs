//! Small Bevy UI pieces shared by the overlay screens: change-quiet component
//! setters, the common text node bundle and the layout operations every screen
//! repeats.

use bevy::prelude::*;

/// Drop shadow behind screen texts so they stay readable over the world.
pub(crate) const TEXT_SHADOW: TextShadow = TextShadow {
    offset: Vec2::new(2.0, 2.0),
    color: Color::BLACK,
};

/// Text node with a font size, color and justification, plus the shared shadow.
#[derive(Bundle)]
pub(crate) struct UiText {
    text: Text,
    font: TextFont,
    color: TextColor,
    layout: TextLayout,
    shadow: TextShadow,
}

impl UiText {
    pub(crate) fn new(text: impl Into<String>, font_size: f32, color: Color) -> Self {
        Self {
            text: Text::new(text),
            font: TextFont {
                font_size: font_size.into(),
                ..default()
            },
            color: TextColor(color),
            layout: TextLayout::default(),
            shadow: TEXT_SHADOW,
        }
    }

    pub(crate) fn justify(mut self, justify: Justify) -> Self {
        self.layout = TextLayout::default().with_justify(justify);
        self
    }
}

pub(crate) fn set_text(text: &mut Text, value: &str) {
    if text.0 != value {
        text.0 = value.to_string();
    }
}

pub(crate) fn set_text_color(color: &mut TextColor, value: Color) {
    if color.0 != value {
        color.0 = value;
    }
}

pub(crate) fn set_font_size(font: &mut TextFont, value: f32) {
    let size = value.into();
    if font.font_size != size {
        font.font_size = size;
    }
}

pub(crate) fn set_display(node: &mut Node, visible: bool) {
    let display = if visible {
        Display::Flex
    } else {
        Display::None
    };
    if node.display != display {
        node.display = display;
    }
}

pub(crate) fn set_visibility(visibility: &mut Visibility, visible: bool) {
    let desired = if visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if *visibility != desired {
        *visibility = desired;
    }
}

/// Clamps the scroll offset so the selected row is fully inside the viewport.
pub(crate) fn follow_selected_row(
    scroll: &mut ScrollPosition,
    computed: &ComputedNode,
    selected: usize,
    row_height: f32,
) {
    let viewport_height = computed.size().y;
    let content_height = computed.content_size().y;
    let selected_top = selected as f32 * row_height;
    let selected_bottom = selected_top + row_height;
    let mut y = scroll.0.y;
    if selected_top < y {
        y = selected_top;
    } else if viewport_height > 0.0 && selected_bottom > y + viewport_height {
        y = selected_bottom - viewport_height;
    }
    let max_scroll = (content_height - viewport_height).max(0.0);
    y = y.clamp(0.0, max_scroll);
    if (scroll.0.y - y).abs() > f32::EPSILON {
        scroll.0.y = y;
    }
}

/// Full-screen, initially hidden node that an overlay screen hangs its panels
/// off.
#[must_use]
pub(crate) fn spawn_gui_root(commands: &mut Commands, marker: impl Component) -> Entity {
    commands
        .spawn((
            marker,
            Visibility::Hidden,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
        ))
        .id()
}
