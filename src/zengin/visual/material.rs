use crate::{warn_unimplemented, zengin::common::MIRROR_X};
use bevy::{prelude::*, render::render_resource::Face};
use std::hash::Hash;
use std::hash::Hasher;
use zen_kit_rs::{
    material::Material,
    misc::{AlphaFunction, MaterialGroup},
};

/// Type for checking for quni materials
/// Used to reduce number of bevy materials used
#[derive(Debug)]
pub struct MatrialHashed(pub StandardMaterial);

impl Hash for MatrialHashed {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_stdandard_material(state, &self.0);
    }
}
impl PartialEq for MatrialHashed {
    fn eq(&self, other: &MatrialHashed) -> bool {
        same_stdandard_material(&self.0, &other.0)
    }
}
impl Eq for MatrialHashed {}

// CAREFUL! list here all data which might change in zengin assets
// If some data will be missed wrong materials might be used on some models
pub fn same_stdandard_material(aa: &StandardMaterial, bb: &StandardMaterial) -> bool {
    aa.flip_normal_map_y == bb.flip_normal_map_y
        && aa.cull_mode == bb.cull_mode
        && aa.perceptual_roughness == bb.perceptual_roughness
        && aa.reflectance == bb.reflectance
        && aa.metallic == bb.metallic
        && aa.specular_tint == bb.specular_tint
        && aa.alpha_mode == bb.alpha_mode
        && aa.base_color_texture == bb.base_color_texture
}

pub fn hash_stdandard_material<H: Hasher>(state: &mut H, mat: &StandardMaterial) {
    mat.flip_normal_map_y.hash(state);
    mat.base_color_texture.hash(state);
    mat.cull_mode.hash(state);
    mat.perceptual_roughness.to_bits().hash(state);
    mat.reflectance.to_bits().hash(state);
    mat.metallic.to_bits().hash(state);
    hash_color(state, &mat.specular_tint);
    hash_alpha(state, &mat.alpha_mode);
}

fn hash_color<H: Hasher>(state: &mut H, color: &Color) {
    let color = color.to_linear();
    color.red.to_bits().hash(state);
    color.green.to_bits().hash(state);
    color.blue.to_bits().hash(state);
    color.alpha.to_bits().hash(state);
}

fn hash_alpha<H: Hasher>(state: &mut H, mode: &AlphaMode) {
    match mode {
        AlphaMode::Opaque => 1.hash(state),
        AlphaMode::Mask(val) => val.to_bits().hash(state),
        AlphaMode::Blend => 2.hash(state),
        AlphaMode::Premultiplied => 3.hash(state),
        AlphaMode::AlphaToCoverage => 4.hash(state),
        AlphaMode::Add => 5.hash(state),
        AlphaMode::Multiply => 6.hash(state),
    }
}

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
        perceptual_roughness: 0.2,
        reflectance: 0.0,
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
        1.0,
    );
    // material.specular_tint = Color::linear_rgba(0.0, 1.0, 0.0, 1.0);
    material
}
