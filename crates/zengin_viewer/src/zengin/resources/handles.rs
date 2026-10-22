//! Cached asset handles shared by all zengin object spawning: materials,
//! images, models, meshes and animations are created once and reused by path.

use std::collections::HashMap;

use bevy::asset::AssetId;
use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use bevy::prelude::*;

use crate::graphics::AnimationData;
use crate::zengin::loaders::model::ZenGinModelLoaderSettings;
use crate::zengin::visual::material::MaterialHashed;
use crate::zengin::visual::model::ZenGinModel;

#[derive(Default, Debug, Hash, PartialEq, Clone, Eq)]
pub struct ModelMeshKey {
    pub model: Handle<ZenGinModel>,
    pub mesh_index: usize,
}

#[derive(Resource, Default)]
pub struct CachedAssetHandles {
    pub materials: HashMap<MaterialHashed, Handle<StandardMaterial>>,
    pub images: HashMap<String, Handle<Image>>,
    pub models: HashMap<String, Handle<ZenGinModel>>,
    pub meshes: HashMap<ModelMeshKey, Handle<Mesh>>,
    pub animations: HashMap<String, Handle<AnimationData>>,
    pub inverse_bindposes: HashMap<AssetId<ZenGinModel>, Handle<SkinnedMeshInverseBindposes>>,
}

impl CachedAssetHandles {
    pub fn get_material_handle(
        &mut self,
        materials: &mut ResMut<Assets<StandardMaterial>>,
        material: &StandardMaterial,
    ) -> Handle<StandardMaterial> {
        if let Some(handle) = self.materials.get(&MaterialHashed(material.clone())) {
            return handle.clone();
        }
        let handle = materials.add(material.clone());
        self.materials
            .insert(MaterialHashed(material.clone()), handle.clone());
        handle
    }

    pub fn get_image_handle(
        &mut self,
        asset_server: &Res<AssetServer>,
        image_path: &str,
    ) -> Handle<Image> {
        if let Some(handle) = self.images.get(image_path) {
            return handle.clone();
        }
        let handle = asset_server.load(image_path.to_string());
        self.images.insert(image_path.to_string(), handle.clone());
        handle
    }

    pub fn get_animation_handle(
        &mut self,
        asset_server: &AssetServer,
        path: &str,
    ) -> Handle<AnimationData> {
        if let Some(handle) = self.animations.get(path) {
            return handle.clone();
        }
        let handle = asset_server.load(path.to_string());
        self.animations.insert(path.to_string(), handle.clone());
        handle
    }

    pub fn get_model_handle(
        &mut self,
        asset_server: &Res<AssetServer>,
        model_path: &str,
        hierarchy_path: Option<&str>,
    ) -> Handle<ZenGinModel> {
        if let Some(handle) = self.models.get(model_path) {
            return handle.clone();
        }

        let hierarchy_path = hierarchy_path.map(std::string::ToString::to_string);

        let handle = asset_server
            .load_builder()
            .with_settings(move |s: &mut ZenGinModelLoaderSettings| {
                s.hierarchy_path.clone_from(&hierarchy_path)
            })
            .load(model_path.to_string());
        self.models.insert(model_path.to_string(), handle.clone());
        handle
    }

    /// Inverse bind poses are derived from the model asset, so the handle is
    /// cached per model. Recreating it on every model (re)spawn would leak a
    /// duplicate asset each time.
    pub fn get_inverse_bindposes_handle(
        &mut self,
        inverse_bindposes: &mut Assets<SkinnedMeshInverseBindposes>,
        model: &ZenGinModel,
        model_handle: &Handle<ZenGinModel>,
    ) -> Handle<SkinnedMeshInverseBindposes> {
        if let Some(handle) = self.inverse_bindposes.get(&model_handle.id()) {
            return handle.clone();
        }
        let handle = inverse_bindposes.add(model.inverse_bindposes.clone());
        self.inverse_bindposes
            .insert(model_handle.id(), handle.clone());
        handle
    }

    pub fn get_material_handle_full(
        &mut self,
        asset_server: &Res<AssetServer>,
        materials: &mut ResMut<Assets<StandardMaterial>>,
        image_path: &str,
        mut material: StandardMaterial,
    ) -> Handle<StandardMaterial> {
        let tex_handle = self.get_image_handle(asset_server, image_path);
        material.base_color_texture = Some(tex_handle.clone());
        self.get_material_handle(materials, &material)
    }

    pub fn get_model_mesh_handle(
        &mut self,
        meshes: &mut ResMut<Assets<Mesh>>,
        model: &ZenGinModel,
        model_handle: &Handle<ZenGinModel>,
        mesh_index: usize,
    ) -> Handle<Mesh> {
        let key = ModelMeshKey {
            model: model_handle.clone(),
            mesh_index,
        };
        if let Some(handle) = self.meshes.get(&key) {
            return handle.clone();
        }
        let handle = meshes.add(model.sub_meshes[mesh_index].mesh.clone());
        self.meshes.insert(key, handle.clone());
        handle
    }
}
