//! Hitpoints of npcs.

use bevy::prelude::*;

/// Hitpoints of an npc (`attribute[ATR_HITPOINTS]`), maintained by the script
/// bridge.
#[derive(Component, Clone, Copy)]
#[allow(dead_code)] // fields are consumed by the health bar UI (next step)
pub struct NpcHealth {
    pub hitpoints: i32,
    pub hitpoints_max: i32,
}

impl NpcHealth {
    pub fn new(hitpoints: i32, hitpoints_max: i32) -> Self {
        Self {
            hitpoints,
            hitpoints_max,
        }
    }
}
