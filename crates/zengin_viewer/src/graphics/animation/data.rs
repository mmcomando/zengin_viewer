use bevy::log::warn;
use bevy::prelude::*;

/// Position and rotation of a bone in a single animation frame.
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

/// Skeletal animation asset loaded from a zengin `.MAN` file.
#[derive(TypePath, Asset, Debug, Clone, Default)]
pub struct AnimationData {
    samples: Vec<AnimationSample>,
    bone_indices: Vec<u32>,
    pub frames_num: u32,
    pub fps: f32,
}

impl AnimationData {
    /// Creates animation data from bone samples ordered frame by frame.
    pub fn new(
        samples: Vec<AnimationSample>,
        bone_indices: Vec<u32>,
        frames_num: u32,
        fps: f32,
    ) -> Self {
        Self {
            samples,
            bone_indices,
            frames_num,
            fps,
        }
    }

    pub fn is_good_animations(&self) -> bool {
        self.frames_num > 1
    }

    pub fn get_bone_sample(&self, frame_index: usize, bone_index: usize) -> AnimationSample {
        let bones_num = self.bone_indices.len();
        if let Some(sample) = self.samples.get(bones_num * frame_index + bone_index) {
            return *sample;
        }
        if !self.samples.is_empty() {
            return self.samples[0];
        }
        warn!(
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

    pub fn get_movement(&self, bone_index: usize) -> Vec<f32> {
        let local_bone_index = self.get_index_for_bone(bone_index).unwrap();

        let mut movement = vec![];

        let start = self.get_bone_sample(0, local_bone_index).position.z;

        for frame_index in 0..self.frames_num as usize {
            let position = self.get_bone_sample(frame_index, local_bone_index).position;
            movement.push(position.z - start);
        }
        return movement;
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn compute_average_min_max_position_for_bone(
        &self,
        bone_index: usize,
    ) -> Option<(Vec3, Vec3, Vec3)> {
        let local_bone_index = self.get_index_for_bone(bone_index)?;
        let bones_num = self.bone_indices.len();
        if bones_num == 0 || self.frames_num == 0 {
            return None;
        }

        let first_sample = self.get_bone_sample(0, local_bone_index);
        let mut position_sum = Vec3::ZERO;
        let mut min_position = first_sample.position;
        let mut max_position = first_sample.position;

        for frame_index in 0..self.frames_num as usize {
            let position = self.get_bone_sample(frame_index, local_bone_index).position;
            position_sum += position;
            min_position = min_position.min(position);
            max_position = max_position.max(position);
        }

        let average_position = position_sum / self.frames_num as f32;
        Some((average_position, min_position, max_position))
    }

    pub fn bone_has_movement(&self, bone_index: usize) -> bool {
        let Some((_average_position, min_position, max_position)) =
            self.compute_average_min_max_position_for_bone(bone_index)
        else {
            return false;
        };

        if max_position.z - min_position.z < 0.4 {
            return false;
        }

        return true;
    }

    /// Root movement of the base pos bone, if the animation moves it
    pub fn root_movement(&self, base_pos_node_index: usize) -> Option<Vec<f32>> {
        if self.bone_has_movement(base_pos_node_index) {
            Some(self.get_movement(base_pos_node_index))
        } else {
            None
        }
    }
}
