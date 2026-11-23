//! On-screen messages requested by `PrintScreen`/`AI_PrintScreen`: centered or
//! percent-positioned texts shown for a few seconds.

use bevy::prelude::*;

use crate::perf_debug;
use crate::zengin::ScreenMessage;

pub struct ScreenMessagesPlugin;

impl Plugin for ScreenMessagesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (handle_screen_messages, expire_screen_messages));
    }
}

/// The engine keeps a message for `time` seconds plus one more.
const EXTRA_DISPLAY_SECONDS: f32 = 1.0;
/// Font size of a message in logical pixels.
const MESSAGE_FONT_SIZE: f32 = 20.0;

/// Marks a spawned on-screen message root with its remaining display time.
#[derive(Component)]
struct ScreenMessageMarker {
    time_left: f32,
}

/// Spawns a full-screen flex box anchoring the message: negative coordinates
/// center the text on that axis, non-negative ones offset it by per cent of
/// the screen size (the engine computes `y` against the text height too, the
/// difference is negligible for the few per cent offsets scripts use).
///
/// The offset goes through `UiTransform`, whose percentages resolve against
/// the node's own axes. Percentage `padding` cannot be used here: its vertical
/// components resolve against the container width, tying the vertical offset
/// to the window width.
fn handle_screen_messages(mut commands: Commands, mut messages: MessageReader<ScreenMessage>) {
    let _perf = perf_debug!();
    for message in messages.read() {
        let seconds = message.time_sec.max(0) as f32 + EXTRA_DISPLAY_SECONDS;
        let horizontal = if message.x < 0 {
            JustifyContent::Center
        } else {
            JustifyContent::Start
        };
        let vertical = if message.y < 0 {
            AlignItems::Center
        } else {
            AlignItems::FlexStart
        };
        let offset = Val2::percent(message.x.max(0) as f32, message.y.max(0) as f32);
        commands
            .spawn((
                ScreenMessageMarker { time_left: seconds },
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: horizontal,
                    align_items: vertical,
                    ..default()
                },
                UiTransform::from_translation(offset),
            ))
            .with_child((
                Text::new(message.text.clone()),
                TextFont {
                    font_size: MESSAGE_FONT_SIZE.into(),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.9, 0.75)),
                TextShadow {
                    offset: Vec2::new(1.0, 1.0),
                    color: Color::BLACK,
                },
            ));
    }
}

fn expire_screen_messages(
    time: Res<Time>,
    mut commands: Commands,
    mut messages: Query<(Entity, &mut ScreenMessageMarker)>,
) {
    let _perf = perf_debug!();
    for (entity, mut message) in &mut messages {
        message.time_left -= time.delta_secs();
        if message.time_left <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}
