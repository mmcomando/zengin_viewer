use bevy::ecs::component::Component;

use crate::world_entity::WorldEntityMarker;

/// The player entity; it is also the player's physics body.
///
/// Its origin is the model root (the feet), not the collider center; the
/// capsule is offset up by `PLAYER_MODEL_ROOT_TO_CAPSULE_CENTER`.
#[derive(Default, Component)]
#[require(WorldEntityMarker)]
pub struct PlayerCharacter;

/// Visual model data of an npc
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

    /// Animation scheme (visual `.mds` stem) used as animation file prefix,
    /// e.g. `HUMANS` or `WOLF`. `None` when it could not be derived.
    pub animation_scheme: Option<String>,
    /// Standing body state animation of [`Self::animation_scheme`], e.g. `S_RUN`
    /// for humans and `S_FISTRUN` for most creatures. `None` when unresolved.
    pub stand_animation: Option<String>,

    /// Body fatness (`Mdl_SetModelFatness`), applied to the torso bone
    pub model_fatness: Option<f32>,
}

/// Vertical offset from an npc entity root to its model root, in world units.
/// Interactive mob attachment points are expressed at the model root, while
/// the npc entity is placed at the model root minus this offset.
#[derive(Debug, Default, Component, Clone, Copy)]
pub struct NpcBodyRootOffset(pub f32);
