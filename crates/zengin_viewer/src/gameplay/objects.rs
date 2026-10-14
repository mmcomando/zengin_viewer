use bevy::ecs::component::Component;

/// Marker component to inform which entity is a player
#[derive(Default, Component)]
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
    /// Humanoids can wear armor
    pub armor_model: Option<String>,

    /// Animation scheme (visual `.mds` stem) used as animation file prefix,
    /// e.g. `HUMANS` or `WOLF`. `None` when it could not be derived.
    pub animation_scheme: Option<String>,
    /// Standing body state animation of [`Self::animation_scheme`], e.g. `S_RUN`
    /// for humans and `S_FISTRUN` for most creatures. `None` when unresolved.
    pub stand_animation: Option<String>,
}
