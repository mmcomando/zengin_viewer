use crate::zengin::common::ZenGinModel;
use crate::zengin::loaders::model::ZenGinModelLoader;
use crate::zengin::loaders::texture::ZenGinTextureLoader;
use crate::zengin::visual::material::MatrialHashed;
use avian3d::prelude::*;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

#[derive(Default)]
pub struct ZenGinInsertResources;

impl Plugin for ZenGinInsertResources {
    fn build(&self, app: &mut App) {
        app.insert_resource(MaterialHandles::default());
        app.init_asset::<ZenGinModel>();
        app.init_asset_loader::<ZenGinTextureLoader>();
        app.init_asset_loader::<ZenGinModelLoader>();

        app.add_systems(
            Update,
            convert_zengin_model_to_entities.run_if(run_convert_zengin_model_to_entities),
        );
    }
}

pub const STATIC_OBJECT: u32 = 1 << 1;
pub const DYNAMIC_OBJECT: u32 = 1 << 2;

/// Adding this component will spawn child entities with 3d meshes contained in `model_handle`
#[derive(Component, Default)]
pub struct ZenGinModelComponent {
    pub model_handle: Handle<ZenGinModel>,
    pub override_texture: Option<String>,
    pub convex_colider: bool,
    pub trimesh_collider: bool,
}

#[derive(Resource, Default)]
pub struct MaterialHandles {
    pub materials: HashMap<MatrialHashed, Handle<StandardMaterial>>,
    pub images: HashMap<String, Handle<Image>>,
    pub models: HashMap<String, Handle<ZenGinModel>>,
}

impl MaterialHandles {
    pub fn get_material_handle(
        &mut self,
        materials: &mut ResMut<Assets<StandardMaterial>>,
        material: &StandardMaterial,
    ) -> Handle<StandardMaterial> {
        if let Some(handle) = self.materials.get(&MatrialHashed(material.clone())) {
            return handle.clone();
        }
        let handle = materials.add(material.clone());
        self.materials
            .insert(MatrialHashed(material.clone()), handle.clone());
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

    pub fn get_model_handle(
        &mut self,
        asset_server: &Res<AssetServer>,
        model_path: &str,
    ) -> Handle<ZenGinModel> {
        if let Some(handle) = self.models.get(model_path) {
            return handle.clone();
        }
        let handle = asset_server.load(model_path.to_string());
        self.models.insert(model_path.to_string(), handle.clone());
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
}

/// Check only entities which were not handled previously
#[derive(Component, Default)]
struct ZenGinModelComponentLoaded {}

fn run_convert_zengin_model_to_entities(
    query: Query<&ZenGinModelComponent, Without<ZenGinModelComponentLoaded>>,
) -> bool {
    query.iter().next().is_some()
}

#[allow(clippy::type_complexity)]
fn convert_zengin_model_to_entities(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    models: ResMut<Assets<ZenGinModel>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut material_handles: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
    query: Query<(Entity, &ZenGinModelComponent), Without<ZenGinModelComponentLoaded>>,
) {
    for (entity_id, model_component) in query.iter() {
        let handle = &model_component.model_handle;
        let Some(model) = models.get(handle) else {
            continue;
        };

        let mut entity = commands.entity(entity_id);
        entity.insert(ZenGinModelComponentLoaded::default());

        for sub_mesh in &model.sub_meshes {
            let texture = if let Some(texture) = &model_component.override_texture {
                texture
            } else {
                &sub_mesh.texture
            };
            let material_handle = material_handles.get_material_handle_full(
                &asset_server,
                &mut materials,
                texture,
                sub_mesh.material.clone(),
            );

            let mesh_handle = meshes.add(sub_mesh.mesh.clone());
            if model_component.convex_colider && sub_mesh.collides {
                entity.with_child((
                    RigidBody::Static,
                    ColliderConstructor::ConvexHullFromMesh,
                    CollisionLayers::from_bits(STATIC_OBJECT, DYNAMIC_OBJECT),
                    sub_mesh.transform,
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(material_handle),
                ));
            } else if model_component.trimesh_collider {
                entity.with_child((
                    RigidBody::Static,
                    ColliderConstructor::TrimeshFromMesh,
                    CollisionLayers::from_bits(STATIC_OBJECT, DYNAMIC_OBJECT),
                    sub_mesh.transform,
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(material_handle),
                ));
            } else {
                entity.with_child((
                    sub_mesh.transform,
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(material_handle),
                ));
            }
        }
    }
}
