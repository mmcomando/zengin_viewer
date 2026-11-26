use bevy::prelude::*;

/// Raw mesh data of one zengin sub mesh, converted into a Bevy [`Mesh`] by the
/// mesh loaders.
#[derive(Debug, Default)]
pub struct MeshData {
    pub indices: Vec<u32>,
    pub vertices: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub normals: Vec<Vec3>,
    pub colors: Vec<Vec4>,

    pub weights: Vec<[f32; 4]>,
    pub bone_indices: Vec<[u16; 4]>,

    pub material: StandardMaterial,
    pub collides: bool,
}

/// One renderable sub mesh of a zengin model with its world placement.
#[derive(Debug, Clone)]
pub struct ZenGinSubMesh {
    pub texture: String,
    pub material: StandardMaterial,
    pub mesh: Mesh,
    pub transform: Transform,
    pub name: String,
    pub collides: bool,
    pub is_skinned: bool,
}

/// Loaded zengin model: sub meshes, node hierarchy and skinning data.
#[derive(Debug, Default, Clone, Asset, TypePath)]
pub struct ZenGinModel {
    pub sub_meshes: Vec<ZenGinSubMesh>,
    pub final_tr: Vec<Transform>,
    pub node_names: Vec<String>,
    pub nodes: Vec<Transform>,
    pub inverse_bindposes: Vec<Mat4>,
    pub parents: Vec<i16>,
    pub animation_bone_index: Vec<usize>,
    /// Hierarchy root offset, in world units. The engine calls this the model
    /// root translation; attachment points are expressed in the space this
    /// offset belongs to.
    pub root_translation: Vec3,
    /// At least one sub mesh is skinned to the node hierarchy. Skinned models
    /// render in raw hierarchy space, so their attachment points have to be
    /// taken without `root_translation`; static meshes already live in model
    /// space and keep it.
    pub has_skinned_meshes: bool,
}
