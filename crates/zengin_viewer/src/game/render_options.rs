use bevy::prelude::*;

/// Runtime-toggleable rendering quality options, edited in the debug UI.
#[derive(Resource)]
pub struct RenderOptions {
    /// Enable sun shadow maps.
    pub sun_shadow_maps: bool,
    /// Enables shadow maps for the disco lamp point lights (very heavy).
    pub disco_shadow_maps: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            sun_shadow_maps: true,
            disco_shadow_maps: false,
        }
    }
}

pub struct RenderOptionsPlugin;

impl Plugin for RenderOptionsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RenderOptions::default());
    }
}
