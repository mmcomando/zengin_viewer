//! Bottom-left log of recently picked up items, each entry shown for a few
//! seconds.

use bevy::prelude::*;

/// How long a single entry stays visible.
const ENTRY_LIFETIME: f32 = 5.0;
/// Maximum number of entries kept, oldest are dropped first.
const MAX_ENTRIES: usize = 8;
/// Font size of a log line in logical pixels.
const LOG_FONT_SIZE: f32 = 16.0;

/// Recently picked up items, each with its remaining display time.
#[derive(Resource, Default)]
pub struct PickupLog {
    entries: Vec<PickupEntry>,
}

struct PickupEntry {
    text: String,
    time_left: f32,
}

impl PickupLog {
    /// Adds an entry, shown for [`ENTRY_LIFETIME`] seconds.
    pub fn add(&mut self, text: impl Into<String>) {
        self.entries.push(PickupEntry {
            text: text.into(),
            time_left: ENTRY_LIFETIME,
        });
        if self.entries.len() > MAX_ENTRIES {
            self.entries.remove(0);
        }
    }
}

pub struct PickupLogPlugin;

impl Plugin for PickupLogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PickupLog>();
        app.add_systems(Startup, spawn_pickup_log);
        app.add_systems(Update, update_pickup_log);
    }
}

/// Marker for the text node showing the log.
#[derive(Component)]
struct PickupLogText;

fn spawn_pickup_log(mut commands: Commands) {
    commands.spawn((
        PickupLogText,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            bottom: Val::Px(12.0),
            ..default()
        },
        Text::new(""),
        TextFont {
            font_size: LOG_FONT_SIZE.into(),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.9, 0.9)),
        TextShadow {
            offset: Vec2::new(1.0, 1.0),
            color: Color::BLACK,
        },
    ));
}

fn update_pickup_log(
    time: Res<Time>,
    mut log: ResMut<PickupLog>,
    mut text: Query<&mut Text, With<PickupLogText>>,
) {
    let delta = time.delta_secs();
    for entry in &mut log.entries {
        entry.time_left -= delta;
    }
    log.entries.retain(|entry| entry.time_left > 0.0);

    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let joined = log
        .entries
        .iter()
        .map(|entry| entry.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if text.0 != joined {
        text.0 = joined;
    }
}
