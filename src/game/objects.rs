use bevy::ecs::component::Component;

#[derive(Debug, Default, Component)]
pub struct GameNpc {
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
