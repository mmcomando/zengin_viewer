//! Killed npcs stay in the world as corpses: their scripts marked them dead,
//! their AI is stopped, and a death animation keeps them lying down.

use bevy::prelude::*;

/// Npc killed in the world. Routine, action and selection systems leave it
/// alone; the animation stays on the death pose.
#[derive(Component)]
pub(crate) struct NpcCorpseMarker;
