//! Debug-only observation systems. Logs here use the `TEST_LOG` target and are
//! derived from existing game messages and change detection, so gameplay code
//! stays unaware of scenario testing.

mod events;
mod npc;

use bevy::prelude::*;

pub(crate) use npc::DebugNpc;

/// Adds the debug observation systems.
pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(npc::DebugNpcPlugin)
            .add_plugins(events::DebugEventsPlugin);
    }
}
