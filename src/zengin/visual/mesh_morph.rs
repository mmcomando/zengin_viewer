use crate::zengin::{common::*, visual::mesh_mrs::meshes_from_zengin_mrs_mesh};

pub fn meshes_from_zengin_morph_mesh(
    morph_mesh: &zen_kit_rs::morph_mesh::MorphMesh,
) -> ZenGinModel {
    let mrs_mesh = morph_mesh.mesh();
    meshes_from_zengin_mrs_mesh(&mrs_mesh, None, &[])
}
