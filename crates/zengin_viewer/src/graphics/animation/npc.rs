use bevy::ecs::component::Component;

/// Animation config of an npc
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
