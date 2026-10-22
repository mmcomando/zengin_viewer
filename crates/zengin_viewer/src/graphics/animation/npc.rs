// TODO too small of a file move to mod, or components
use bevy::ecs::component::Component;

/// Animation config of an npc
#[derive(Debug, Default, Component)]
pub struct NpcAnimations {
    /// Animation the entity loops; `None` falls back to the stand animation
    /// of the model scheme
    pub animation: Option<String>,
}
