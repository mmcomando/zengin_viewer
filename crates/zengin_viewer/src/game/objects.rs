use bevy::ecs::component::Component;

use crate::zengin::script::symbol_id::SymbolId;

/// Identifies the npc's script instance.
#[derive(Debug, Component)]
pub struct GameNpc {
    pub npc_instance_id: SymbolId,
}

/// Visual model data of an npc, requires [`GameNpc`] on the same entity.
#[derive(Debug, Default, Component)]
pub struct NpcModel {
    /// Bone hierarchy path
    pub hierarchy: Option<String>,

    pub body_model: String,
    /// Override default body texture
    pub body_texture: Option<String>,

    /// Humans and some monsters have separate head model attached to "BIP01 HEAD" body node
    pub head_model: Option<String>,
    /// Override default head texture
    pub head_texture: Option<String>,
    /// Humanoids can wear armor
    pub armor_model: Option<String>,
}

/// Animation config of an npc, requires [`GameNpc`] on the same entity.
#[derive(Debug, Default, Component)]
pub struct NpcAnimations {
    /// Animation played when [`Self::movement_animations`] is not set,
    /// otherwise used as fallback when the entity has no movement animations
    pub animation: Option<String>,
    /// When set, the animation controller switches between those animations
    /// based on entity movement
    pub movement_animations: Option<MovementAnimations>,
}

/// Animations driving entity movement. Transitions are played once, the others loop.
#[derive(Debug, Clone)]
pub struct MovementAnimations {
    /// Played while the entity does not move
    pub idle: String,
    /// Played once when the entity starts moving
    pub move_start: Option<String>,
    /// Played while the entity moves
    pub move_loop: String,
    /// Played once when the entity stops moving
    pub move_stop: Option<String>,
}
