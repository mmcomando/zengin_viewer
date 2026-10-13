use std::collections::HashMap;

use crate::game::animation::AnimationData;
use crate::game::distance::{ClassifyDistance, DistanceFar, DistanceNear};
use crate::zengin::common::ZenGinModel;
use crate::zengin::loaders::animation::ZenGinAnimationLoader;
use crate::zengin::loaders::model::{ZenGinModelLoader, ZenGinModelLoaderSettings};
use crate::zengin::loaders::texture::ZenGinTextureLoader;
use crate::zengin::visual::material::MatrialHashed;
use avian3d::prelude::*;
use bevy::camera::primitives::{Aabb, MeshAabb};
use bevy::camera::visibility::DynamicSkinnedMeshBounds;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;

#[derive(Default)]
pub struct ZenGinInsertResources;

impl Plugin for ZenGinInsertResources {
    fn build(&self, app: &mut App) {
        app.insert_resource(MaterialHandles::default());
        app.init_asset::<ZenGinModel>();
        app.init_asset::<AnimationData>();

        app.init_asset_loader::<ZenGinTextureLoader>();
        app.init_asset_loader::<ZenGinModelLoader>();
        app.init_asset_loader::<ZenGinAnimationLoader>();

        app.add_systems(
            Update,
            convert_zengin_model_to_entities.run_if(run_convert_zengin_model_to_entities),
        );

        app.add_systems(Update, add_classify_distance);
        app.add_systems(Last, despawn_visual_when_far_resource);
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
    pub bones_data: Option<SkinnedMesh>,
}

#[derive(Default, Debug, Hash, PartialEq, Clone, Eq)]
pub struct ModelMeshKey {
    pub model: Handle<ZenGinModel>,
    pub mesh_index: usize,
}

#[derive(Resource, Default)]
pub struct MaterialHandles {
    pub materials: HashMap<MatrialHashed, Handle<StandardMaterial>>,
    pub images: HashMap<String, Handle<Image>>,
    pub models: HashMap<String, Handle<ZenGinModel>>,
    pub meshes: HashMap<ModelMeshKey, Handle<Mesh>>,
    pub animations: HashMap<String, Handle<AnimationData>>,
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

    pub fn get_animation_handle(
        &mut self,
        asset_server: &Res<AssetServer>,
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

/// Check only entities which were not handled previously
#[derive(Component, Default)]
pub struct ZenGinModelComponentLoaded {}

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
    query: Query<
        (Entity, &ZenGinModelComponent),
        (With<DistanceNear>, Without<ZenGinModelComponentLoaded>),
    >,
) {
    for (entity_id, model_component) in query.iter() {
        let handle = &model_component.model_handle;
        let Some(model) = models.get(handle) else {
            continue;
        };
        let main_entity_id = commands.entity(entity_id).id();
        {
            let mut main_entity = commands.entity(entity_id);
            main_entity.insert(ZenGinModelComponentLoaded::default());
        }

        let mut aabb = Aabb::from_min_max(Vec3::ONE * -1.0, Vec3::ONE);
        for (sub_mesh_index, sub_mesh) in model.sub_meshes.iter().enumerate() {
            if let Some(sub_aabb) = sub_mesh.mesh.compute_aabb() {
                aabb = Aabb::from_min_max(
                    sub_aabb.min().min(aabb.min()).into(),
                    sub_aabb.max().max(aabb.max()).into(),
                );
            }
            if sub_mesh.is_skinned && model_component.bones_data.is_none() {
                // Bevy can't show skinned meshes without SkinnedMesh component
                // https://github.com/bevyengine/bevy/issues/22469
                warn_once!("Skinned static objects are not rendered");
                continue;
            }

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

            let mesh_handle =
                material_handles.get_model_mesh_handle(&mut meshes, model, handle, sub_mesh_index);

            let mut sub_entity = commands.spawn((
                sub_mesh.transform,
                Mesh3d(mesh_handle),
                MeshMaterial3d(material_handle),
                ClassifyDistance::default(),
            ));

            if model_component.convex_colider && sub_mesh.collides {
                sub_entity.insert((
                    RigidBody::Static,
                    ColliderConstructor::ConvexHullFromMesh,
                    CollisionLayers::from_bits(STATIC_OBJECT, DYNAMIC_OBJECT),
                ));
            } else if model_component.trimesh_collider {
                sub_entity.insert((
                    RigidBody::Static,
                    ColliderConstructor::TrimeshFromMesh,
                    CollisionLayers::from_bits(STATIC_OBJECT, DYNAMIC_OBJECT),
                ));
            }

            if sub_mesh.is_skinned
                && let Some(skinned_mesh) = model_component.bones_data.clone()
            {
                sub_entity.insert(skinned_mesh);
                sub_entity.insert(Transform::IDENTITY);
                sub_entity.insert(DynamicSkinnedMeshBounds);
            }

            let sub_entity = sub_entity.id();
            commands.entity(main_entity_id).add_child(sub_entity);
        }
        {
            let mut main_entity = commands.entity(entity_id);
            main_entity.insert(aabb);
        }
    }
}

fn add_classify_distance(
    mut commands: Commands,
    query: Query<Entity, Added<ZenGinModelComponent>>,
) {
    for entity_id in query {
        commands
            .entity(entity_id)
            .insert(ClassifyDistance::default());
    }
}

pub fn despawn_visual_when_far_resource(
    mut commands: Commands,
    q_parent: Query<Entity, (With<DistanceFar>, With<ZenGinModelComponentLoaded>)>,
) {
    for entity_id in q_parent.iter() {
        let mut main_entity = commands.entity(entity_id);
        main_entity.remove::<ZenGinModelComponentLoaded>();
        main_entity.despawn_children();
    }
}
