use bevy::{asset::io::Reader, prelude::*};
use zen_kit_rs::stream::Read;

use bevy::{
    asset::{AssetLoader, LoadContext},
    reflect::TypePath,
};

use crate::zengin::{
    common::ZenGinModel,
    visual::{
        mesh::meshes_from_zengin_mesh,
        mesh_model::{meshes_from_zengin_model, meshes_from_zengin_model_mesh},
        mesh_morph::meshes_from_zengin_morph_mesh,
        mesh_mrs::meshes_from_zengin_mrs_mesh,
    },
};

#[derive(Default, TypePath)]
pub struct ZenGinModelLoader;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ZenGinModelLoaderSettings {
    pub hierarchy_path: Option<String>,
}

impl AssetLoader for ZenGinModelLoader {
    type Asset = ZenGinModel;
    type Settings = ZenGinModelLoaderSettings;
    type Error = BevyError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &ZenGinModelLoaderSettings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let path = load_context.path();
        let path_str = path.to_string();
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        // println!("Load model({:?})", path_str);

        let model = if path_str.ends_with(".MRM") {
            let read = Read::from_slice(&bytes).unwrap();
            let mesh = zen_kit_rs::mrs_mesh::MrsMesh::load(&read).unwrap();
            meshes_from_zengin_mrs_mesh(&mesh, None, &[])
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
            let hierarchy_path = settings
                .hierarchy_path
                .clone()
                .unwrap_or_else(|| path_str.replace("MDM", "MDH"));

            let model_hierarchy =
                if let Ok(hierarchy_bytes) = load_context.read_asset_bytes(&hierarchy_path).await {
                    // println!("Load hierarchy({:?})", hierarchy_path);
                    let hierarchy_read = Read::from_slice(&hierarchy_bytes).unwrap();
                    zen_kit_rs::model::Model::load(&hierarchy_read)
                } else {
                    // println!("No hierarchy({:?})", path_str);
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

        // println!("Model loaded({:?})", path_str);

        Ok(model)
    }

    fn extensions(&self) -> &[&str] {
        &["MRM", "MDL", "MMB", "MDM", "MSB", "MDH", "MSH"]
    }
}
