//! Scenario-visible logs for interactions that are not npc-scoped, derived
//! only from existing game messages.

use bevy::prelude::*;

use crate::audio::PlaySound;
use crate::gui::{LockpickOpened, LootSource, OpenLockpick, OpenLoot};
use crate::test_log;
use crate::zengin::dialog::DialogRequest;

/// Logs lockpick, loot and dialog events with the `TEST_LOG` target.
pub(crate) struct DebugEventsPlugin;

impl Plugin for DebugEventsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                log_lockpick_started,
                log_lockpick_completed,
                log_lockpick_failure,
                log_loot_opened,
                log_dialog_opened,
            ),
        );
    }
}

fn log_lockpick_started(mut messages: MessageReader<OpenLockpick>) {
    for event in messages.read() {
        test_log!(
            "Player({}) started lockpicking chest({})",
            event.player,
            event.chest
        );
    }
}

fn log_lockpick_completed(mut messages: MessageReader<LockpickOpened>) {
    for event in messages.read() {
        test_log!("Lockpick of chest({}) completed", event.chest);
        test_log!("opened lockpicked chest({})", event.chest);
    }
}

fn log_lockpick_failure(mut messages: MessageReader<PlaySound>) {
    for event in messages.read() {
        if event.asset_path.to_uppercase().contains("PICKLOCK_FAILURE") {
            test_log!("Lockpick step was wrong, progress reset");
        }
    }
}

fn log_loot_opened(mut messages: MessageReader<OpenLoot>) {
    for event in messages.read() {
        match event.source {
            LootSource::Chest(chest) => {
                test_log!("Player({}) opened chest({chest})", event.player);
                test_log!("Loot opened for chest({chest})");
            }
            LootSource::Corpse(corpse) => {
                test_log!("Player({}) opened corpse({corpse})", event.player);
                test_log!("Loot opened for corpse({corpse})");
            }
        }
    }
}

fn log_dialog_opened(mut messages: MessageReader<DialogRequest>) {
    for event in messages.read() {
        if let DialogRequest::Open {
            npc_from,
            npc_talk_to,
            ..
        } = event
        {
            test_log!("Dialog open npc({npc_talk_to}) player({npc_from})");
        }
    }
}
