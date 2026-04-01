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

const HUMAN_MODEL_HIERARCHY: &str = "zengin://_WORK/DATA/ANIMS/_COMPILED/HUMANS.MDH";

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
            let mut hierarchy_path = path_str.replace("MDM", "MDH");

            // TODO findout how to select model from which skeleton/hierarchy is taken for given model
            warn_once!("Choosing of model hierarchy for some models is hardcoded");
            let use_human_hierarchy: &[&str] = &[
                "zengin://_WORK/DATA/ANIMS/_COMPILED/HUM_BODY_NAKED0.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_PAL_M.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_PAL_H.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_VLKBABE_H.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_VLKBABE_M.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_BAUBABE_L.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_BARKEEPER.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_BAU_L.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_BAU_M.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_DIEGO.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_GOVERNOR.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_NOV_L.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_PIR_H_ADDON.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_PIR_L_ADDON.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_RANGER_ADDON.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_VLK_M.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_VLK_H.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_KDW_H.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/LUR_BODY.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/STG_BODY.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_PAL_SKELETON.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_MIL_M.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/GIANT_RAT_BODY.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/WAR_BODY.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ORC_BODYWARRIOR.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_MIL_L.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_VLKBABE_L.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/SKE_BODY.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/IRRLICHT_BODY.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/GIANT_BUG_BODY.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/MOL_BODY.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/KEILER_BODY.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ORC_BODYELITE.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ORC_BODYSHAMAN.MDM",
                "zengin://_WORK/DATA/ANIMS/_COMPILED/ARMOR_BAUBABE_M.MDM",
            ];
            if use_human_hierarchy.contains(&path_str.as_str()) {
                hierarchy_path = HUMAN_MODEL_HIERARCHY.to_string();
            }

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
