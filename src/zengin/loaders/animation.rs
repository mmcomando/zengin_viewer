use bevy::{asset::io::Reader, prelude::*};
use zen_kit_rs::stream::Read;

use bevy::{
    asset::{AssetLoader, LoadContext},
    reflect::TypePath,
};

use crate::zengin::common::{get_world_pos, get_world_quat};

#[derive(Default, TypePath)]
pub struct ZenGinAnimationLoader;

#[derive(Debug, Clone, Copy)]
pub struct AnimationSample {
    pub position: Vec3,
    pub rotation: Quat,
}

impl Default for AnimationSample {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        }
    }
}

#[derive(TypePath, Asset, Debug, Clone, Default)]
pub struct AnimationData {
    samples: Vec<AnimationSample>,
    bone_indices: Vec<u32>,
    pub frames_num: u32,
}

impl AnimationData {
    pub fn get_bone_sample(&self, frame_index: usize, bone_index: usize) -> AnimationSample {
        let bones_num = self.bone_indices.len();
        if let Some(sample) = self.samples.get(bones_num * frame_index + bone_index) {
            return *sample;
        }
        if !self.samples.is_empty() {
            return self.samples[0];
        }
        println!(
            "Not found sample for frame_index({frame_index}), bone_index({bone_index}), all samples({}), bones_num({}), ii({})",
            self.samples.len(),
            bones_num,
            bones_num * frame_index + bone_index
        );
        return AnimationSample::default();
    }

    pub fn get_index_for_bone(&self, bone_index: usize) -> Option<usize> {
        self.bone_indices
            .iter()
            .position(|el| *el == bone_index as u32)
    }
}

impl AssetLoader for ZenGinAnimationLoader {
    type Asset = AnimationData;
    type Settings = ();
    type Error = BevyError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let path = load_context.path();
        let path_str = path.to_string();
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
                position: get_world_pos(el.position),
                rotation: get_world_quat(el.rotation),
            })
            .collect();
        assert_eq!(samples.len(), (bones_num * frames_num) as usize);
        let node_indices = animation.node_indices();
        let data = AnimationData {
            samples,
            frames_num,
            bone_indices: Vec::from(node_indices),
        };

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
