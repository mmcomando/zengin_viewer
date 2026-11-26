mod lighting;
mod skybox;

pub(crate) use crate::graphics::lighting::LightingPlugin;
pub(crate) use crate::graphics::skybox::SkyBoxPlugin;

use bevy::prelude::*;

/// Runtime-toggleable rendering quality options, edited in the debug UI.
#[derive(Resource)]
pub struct RenderOptions {
    /// Enable sun shadow maps.
    pub sun_shadow_maps: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            sun_shadow_maps: true,
        }
    }
}

pub struct RenderOptionsPlugin;

impl Plugin for RenderOptionsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RenderOptions::default());
    }
}
