//! Zengin resources: asset loading setup, cached handles and model entity
//! spawning systems.

mod handles;
mod model_spawn;

use bevy::prelude::*;

use crate::graphics::AnimationData;
use crate::zengin::loaders::animation::ZenGinAnimationLoader;
use crate::zengin::loaders::model::ZenGinModelLoader;
use crate::zengin::loaders::texture::ZenGinTextureLoader;
use crate::zengin::visual::model::ZenGinModel;

pub(crate) use handles::{CachedAssetHandles, NamedModels};
pub(crate) use model_spawn::{
    ARROW, DYNAMIC_OBJECT, NPC_BODY, SELECTABLE, STATIC_OBJECT, WAYPOINT, ZenGinModelComponent,
    despawn_visual_when_far_resource,
};

use crate::game_state::AppState;

#[derive(Default)]
pub struct ZenGinInsertResources;

impl Plugin for ZenGinInsertResources {
    fn build(&self, app: &mut App) {
        app.insert_resource(CachedAssetHandles::default());
        app.init_resource::<NamedModels>();
        app.init_asset::<ZenGinModel>();
        app.init_asset::<AnimationData>();

        app.init_asset_loader::<ZenGinTextureLoader>();
        app.init_asset_loader::<ZenGinModelLoader>();
        app.init_asset_loader::<ZenGinAnimationLoader>();

        app.add_systems(
            Update,
            model_spawn::convert_zengin_model_to_entities
                .run_if(model_spawn::run_convert_zengin_model_to_entities)
                .run_if(in_state(AppState::InGame)),
        );

        app.add_systems(
            Update,
            model_spawn::add_classify_distance.run_if(in_state(AppState::InGame)),
        );
        app.add_systems(
            Last,
            despawn_visual_when_far_resource.run_if(in_state(AppState::InGame)),
        );
    }
}
