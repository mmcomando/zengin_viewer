use crate::graphics::{AnimationData, AnimationSample};
use crate::zengin::conversions::*;
use bevy::{asset::io::Reader, prelude::*};
use zen_kit_rs::stream::Read;

use bevy::{
    asset::{AssetLoader, LoadContext},
    reflect::TypePath,
};

use crate::zengin::common::{get_world_pos, get_world_quat};

#[derive(Default, TypePath)]
pub struct ZenGinAnimationLoader;

impl AssetLoader for ZenGinAnimationLoader {
    type Asset = AnimationData;
    type Settings = ();
    type Error = BevyError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        // let path = load_context.path();
        // let path_str = path.to_string();
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        // println!("Load animation({:?})", path_str);

        let read = Read::from_slice(&bytes).unwrap();
        let animation = zen_kit_rs::model_animation::ModelAnimation::load(&read).unwrap();
        let bones_num = animation.node_count();
        let frames_num = animation.frame_count();
        let samples: Vec<_> = animation
            .samples()
            .iter()
            .map(|el| AnimationSample {
                position: get_world_pos(el.position.to_glam()),
                rotation: get_world_quat(el.rotation.to_glam()),
            })
            .collect();
        assert_eq!(samples.len(), (bones_num * frames_num) as usize);
        let node_indices = animation.node_indices();
        let data = AnimationData::new(
            samples,
            Vec::from(node_indices),
            frames_num,
            animation.fps(),
        );

        // println!(
        //     "Animation loaded({:?}) next({}), layer({})",
        //     path_str,
        //     animation.next(),
        //     animation.layer()
        // );

        Ok(data)
    }

    fn extensions(&self) -> &[&str] {
        &["MAN"]
    }
}
