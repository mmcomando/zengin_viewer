use bevy::{prelude::*, render::render_resource::Face};
use zen_kit_rs::{
    material::Material,
    misc::{AlphaFunction, MaterialGroup},
};

use crate::{warn_unimplemented, zengin::common::MIRROR_X};

pub fn get_standard_material(mat: &Material) -> StandardMaterial {
    let cull_mode = if MIRROR_X {
        Some(Face::Back)
    } else {
        Some(Face::Front)
    };
    let mut material = StandardMaterial {
        base_color_texture: None, // Will be set during bevy entity creation
        alpha_mode: AlphaMode::Mask(0.5),
        cull_mode,
        perceptual_roughness: 0.8,
        reflectance: 0.2,
        metallic: 0.0,
        specular_tint: Color::BLACK,
        ..default()
    };

    match mat.alpha_function() {
        AlphaFunction::DEFAULT |
        // NONE seems like it shouldbe Opaque but there are transparent objects with NONE
        // AlphaFunction::NONE => material.alpha_mode = AlphaMode::Opaque
        AlphaFunction::NONE => {}
        AlphaFunction::BLEND => material.alpha_mode = AlphaMode::Blend,
        AlphaFunction::ADD => material.alpha_mode = AlphaMode::Add,
        AlphaFunction::SUBTRACT => warn_unimplemented!("AlphaFunction::SUBTRACT"),
        AlphaFunction::MULTIPLY => material.alpha_mode = AlphaMode::Multiply,
        AlphaFunction::MULTIPLY_ALT => warn_unimplemented!("AlphaFunction::MULTIPLY_ALT"),
    }

    match mat.group() {
        MaterialGroup::METAL => {
            // Most objects in group METAL are more like dielectrics, or maybe this can be configured better?
            // material.metallic = 1.0;
            material.perceptual_roughness = 0.3;
            material.reflectance = 0.6;
        }
        MaterialGroup::STONE => {
            material.perceptual_roughness = 0.7;
            material.reflectance = 0.3;
        }
        MaterialGroup::WOOD => {
            material.perceptual_roughness = 0.6;
            material.reflectance = 0.3;
        }
        MaterialGroup::WATER => {
            material.perceptual_roughness = 0.3;
            material.reflectance = 0.7;
        }
        MaterialGroup::SNOW => {
            material.perceptual_roughness = 0.7;
            material.reflectance = 0.5;
        }
        MaterialGroup::EARTH => {
            material.perceptual_roughness = 0.9;
            material.reflectance = 0.15;
        }
        MaterialGroup::UNDEFINED => {}
    }

    let color = mat.color();
    material.specular_tint = Color::linear_rgba(
        f32::from(color.x) / 255.0,
        f32::from(color.y) / 255.0,
        f32::from(color.z) / 255.0,
        f32::from(color.w) / 255.0,
    );
    material
}
