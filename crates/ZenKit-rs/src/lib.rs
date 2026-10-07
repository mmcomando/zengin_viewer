#![allow(
    clippy::wildcard_imports,
    clippy::uninit_vec,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::missing_safety_doc,
    clippy::borrow_as_ptr,
    clippy::new_without_default,
    clippy::cast_possible_truncation,
    clippy::needless_return
)]

pub mod conversions;
pub mod cutscene_library;
pub mod material;
pub mod mesh;
pub mod misc;
pub mod model;
pub mod model_animation;
pub mod model_scripts;
pub mod morph_mesh;
pub mod mrs_mesh;
pub mod obb;
pub mod stream;
pub mod texture;
pub mod vfs;
pub mod vobs;
pub mod waynet;
pub mod world;

#[cfg(test)]
pub mod testing_common;
