use bevy::{asset::io::Reader, prelude::*};
use zen_kit_rs::stream::Read;

use bevy::{
    asset::{AssetLoader, LoadContext},
    reflect::TypePath,
};

use crate::zengin::{
    common::{ZenGinModel, to_asset_path},
    visual::{
        mesh::meshes_from_zengin_mesh,
        mesh_model::{meshes_from_zengin_model, meshes_from_zengin_model_mesh},
        mesh_morph::meshes_from_zengin_morph_mesh,
        mesh_mrs::meshes_from_zengin_mrs_mesh,
    },
};

const HUMAN_MODEL: &str = "zengin://_WORK/DATA/ANIMS/_COMPILED/HUM_BODY_NAKED0.MDM";
const HUMAN_MODEL_HIERARCHY: &str = "/_WORK/DATA/ANIMS/_COMPILED/HUMANS_RELAXED.MDH";

#[derive(Default, TypePath)]
pub struct ZenGinModelLoader;

impl AssetLoader for ZenGinModelLoader {
    type Asset = ZenGinModel;
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

        // println!("Asset Path: {}", path);

        let model = if path_str.ends_with(".MRM") {
            let read = Read::from_slice(&bytes).unwrap();
            let mesh = zen_kit_rs::mrs_mesh::MrsMesh::load(&read).unwrap();
            meshes_from_zengin_mrs_mesh(&mesh)
        } else if path_str.ends_with(".MDL") {
            let read = Read::from_slice(&bytes).unwrap();
            let mesh = zen_kit_rs::model::Model::load(&read).unwrap();
            meshes_from_zengin_model(&mesh)
        } else if path_str.ends_with(".MMB") {
            let read = Read::from_slice(&bytes).unwrap();
            let mesh = zen_kit_rs::morph_mesh::MorphMesh::load(&read).unwrap();
            meshes_from_zengin_morph_mesh(&mesh)
        } else if path_str.ends_with(".MDM") {
            // We try to load model only file, but maybe there is coresponding hierarchy file
            // If we have hierarchy file load it and use it
            let mut hierarchy_path = path_str.replace("MDM", "MDH");
            if path_str == HUMAN_MODEL {
                hierarchy_path = HUMAN_MODEL_HIERARCHY.to_string();
            }
            let hierarchy_path = to_asset_path(&hierarchy_path);
            let model_hierarchy =
                if let Ok(hierarchy_bytes) = load_context.read_asset_bytes(hierarchy_path).await {
                    let hierarchy_read = Read::from_slice(&hierarchy_bytes).unwrap();
                    zen_kit_rs::model::Model::load(&hierarchy_read)
                } else {
                    None
                };
            let mesh = {
                let read = Read::from_slice(&bytes).unwrap();
                zen_kit_rs::model::ModelMesh::load(&read).unwrap()
            };

            meshes_from_zengin_model_mesh(&mesh, model_hierarchy.as_ref())
        } else if path_str.ends_with(".MSB") || path_str.ends_with(".MDH") {
            let read = Read::from_slice(&bytes).unwrap();
            let mesh = zen_kit_rs::model::Model::load(&read).unwrap();
            meshes_from_zengin_model(&mesh)
        } else if path_str.ends_with(".MSH") {
            let read = Read::from_slice(&bytes).unwrap();
            let mesh = zen_kit_rs::mesh::Mesh::load(&read).unwrap();
            meshes_from_zengin_mesh(&mesh)
        } else {
            return Err(BevyError::from(format!(
                "ZenGinModelLoader mesh_path({}) unrecognized mesh format",
                path_str
            )));
        };

        Ok(model)
    }

    fn extensions(&self) -> &[&str] {
        &["MRM", "MDL", "MMB", "MDM", "MSB", "MDH", "MSH"]
    }
}
