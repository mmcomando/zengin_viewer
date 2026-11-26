//! Killed npcs stay in the world as corpses: their scripts marked them dead,
//! their AI is stopped, and a death animation keeps them lying down. The corpse
//! keeps its inventory and is looted through the shared inventory window.

use bevy::prelude::*;

/// Npc killed in the world. Routine, action and combat systems leave it alone;
/// the animation stays on the death pose and selection offers it as a loot
/// source.
#[derive(Component)]
pub struct NpcCorpseMarker;
