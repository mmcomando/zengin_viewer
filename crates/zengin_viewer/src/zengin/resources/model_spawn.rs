//! Spawning of zengin model visuals: the [`ZenGinModelComponent`] marker,
//! collision layers and the systems turning a model handle into mesh child
//! entities.

use avian3d::prelude::*;
use bevy::camera::primitives::{Aabb, MeshAabb};
use bevy::camera::visibility::NoAutoAabb;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::prelude::*;

use crate::gameplay::character::QUERY_ONLY_LAYER;
use crate::gameplay::distance::{ClassifyDistance, DistanceFar, DistanceNear};
use crate::graphics::{AnimationController, AnimationData};
use crate::gui_debug::WorldMesh;
use crate::zengin::objects_to_entities::create_skinned_mesh_data;
use crate::zengin::resources::handles::CachedAssetHandles;
use crate::zengin::visual::model::ZenGinModel;
use dev_util::perf_debug;
use dev_util::warn_once;

pub const STATIC_OBJECT: u32 = 1 << 1;
pub const DYNAMIC_OBJECT: u32 = 1 << 2;
/// Collision layer of colliders that only exist for spatial queries (selection),
/// they collide with nothing.
pub const SELECTABLE: u32 = QUERY_ONLY_LAYER;
/// Collision layer of the npc hit volumes, used by flying arrows (sensors) to
/// detect hits on npcs.
pub const NPC_BODY: u32 = 1 << 4;
/// Collision layer of arrow sensors.
pub const ARROW: u32 = 1 << 5;
/// Collision layer of waypoint sphere colliders, used only by nearest-waypoint
/// spatial queries, they collide with nothing.
pub const WAYPOINT: u32 = 1 << 6;

/// Margin added to a static culling [`Aabb`] of a skinned mesh, in meters, so
/// animated limbs are not culled near the screen edge.
const SKINNED_MESH_BOUNDS_MARGIN: f32 = 2.0;

/// Adding this component will spawn child entities with 3d meshes contained in `model_handle`
#[derive(Component, Default)]
pub struct ZenGinModelComponent {
    pub model_handle: Handle<ZenGinModel>,
    pub override_texture: Option<String>,
    pub convex_colider: bool,
    pub trimesh_collider: bool,
    pub bones_data: Option<SkinnedMesh>,
    /// Animation played by the skeleton the model is turned into, `None` for a
    /// skeleton frozen in its bind pose.
    pub animation_handle: Option<Handle<AnimationData>>,
}

/// Check only entities which were not handled previously
#[derive(Component, Default)]
pub struct ZenGinModelComponentLoaded {}

pub(crate) fn run_convert_zengin_model_to_entities(
    query: Query<&ZenGinModelComponent, Without<ZenGinModelComponentLoaded>>,
) -> bool {
    query.iter().next().is_some()
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn convert_zengin_model_to_entities(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    models: ResMut<Assets<ZenGinModel>>,
    animations: Res<Assets<AnimationData>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut material_handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut skinned_mesh_inverse_bindposes_assets: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    query: Query<
        (Entity, &ZenGinModelComponent),
        (
            Or<(With<DistanceNear>, With<WorldMesh>)>,
            Without<ZenGinModelComponentLoaded>,
        ),
    >,
) {
    let _perf = perf_debug!();
    for (entity_id, model_component) in query.iter() {
        let handle = &model_component.model_handle;
        let Some(model) = models.get(handle) else {
            continue;
        };
        // Only models which build their own skeleton wait for the animation, a
        // supplied `bones_data` already carries one.
        let animation_data = match (
            &model_component.animation_handle,
            &model_component.bones_data,
        ) {
            (Some(animation_handle), None) => {
                let Some(animation_data) = animations.get(animation_handle) else {
                    continue;
                };
                Some(animation_data)
            }
            _ => None,
        };
        let main_entity_id = commands.entity(entity_id).id();
        {
            let mut main_entity = commands.entity(entity_id);
            main_entity.insert(ZenGinModelComponentLoaded::default());
        }

        // Models like BSFIRE_OC.MDM have their visual partially as soft skin
        // mesh. Bevy can't render skinned meshes without SkinnedMesh component,
        // so for static objects spawn a skeleton frozen in bind pose.
        let has_skinned = model.sub_meshes.iter().any(|el| el.is_skinned);
        let mut bones_data = model_component.bones_data.clone();
        if has_skinned && bones_data.is_none() && !model.nodes.is_empty() {
            bones_data = create_skinned_mesh_data(
                &mut commands,
                &mut material_handles,
                &mut skinned_mesh_inverse_bindposes_assets,
                handle,
                entity_id,
                model,
                animation_data,
            );
            if animation_data.is_some()
                && let Some(animation_handle) = &model_component.animation_handle
            {
                commands
                    .entity(entity_id)
                    .insert(AnimationController::new(animation_handle.clone()));
            }
        }

        let model_aabb = model
            .sub_meshes
            .iter()
            .filter_map(|sub_mesh| sub_mesh.mesh.get_aabb())
            .fold(
                Aabb::from_min_max(Vec3::ONE * -1.0, Vec3::ONE),
                |aabb, sub_aabb| {
                    Aabb::from_min_max(
                        sub_aabb.min().min(aabb.min()).into(),
                        sub_aabb.max().max(aabb.max()).into(),
                    )
                },
            );
        // Skinned poses can leave the bind-pose bounds. A static, expanded box
        // avoids Bevy recomputing skinned bounds from all joints every frame.
        let skinned_aabb = Aabb::from_min_max(
            Vec3::from(model_aabb.min()) - Vec3::splat(SKINNED_MESH_BOUNDS_MARGIN),
            Vec3::from(model_aabb.max()) + Vec3::splat(SKINNED_MESH_BOUNDS_MARGIN),
        );
        for (sub_mesh_index, sub_mesh) in model.sub_meshes.iter().enumerate() {
            if sub_mesh.is_skinned && bones_data.is_none() {
                warn_once!(
                    "Skinned object without hierarchy nodes is not rendered, model({})",
                    model.node_names.first().map_or(String::new(), Clone::clone)
                );
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

            if sub_mesh.collides
                && (model_component.convex_colider || model_component.trimesh_collider)
            {
                sub_entity.insert((
                    RigidBody::Static,
                    // If it is good for perf we can set it for small meshes
                    // ColliderConstructor::ConvexHullFromMesh,
                    ColliderConstructor::TrimeshFromMesh,
                    CollisionLayers::from_bits(STATIC_OBJECT, DYNAMIC_OBJECT),
                ));
            }

            if sub_mesh.is_skinned
                && let Some(skinned_mesh) = bones_data.clone()
            {
                sub_entity.insert(skinned_mesh);
                // A skinned mesh renders at `joint_global * inverse_bind * vertex`,
                // and the bind poses carry the hierarchy root translation, so the
                // visible mesh sits `-root_translation` from the baked vertices.
                // Skinning ignores this entity's transform, but the collider and
                // the culling box do not, so offset them into the rendered space.
                sub_entity.insert(Transform::from_translation(-model.root_translation));
                // The bounds are static, so `calculate_bounds` must not replace
                // the expanded box with the bind-pose one.
                sub_entity.insert((skinned_aabb, NoAutoAabb));
            }

            let sub_entity = sub_entity.id();
            commands.entity(main_entity_id).add_child(sub_entity);
        }
        {
            let mut main_entity = commands.entity(entity_id);
            main_entity.insert(model_aabb);
        }
    }
}

pub(crate) fn add_classify_distance(
    mut commands: Commands,
    query: Query<Entity, Added<ZenGinModelComponent>>,
) {
    let _perf = perf_debug!();
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
    let _perf = perf_debug!();
    for entity_id in q_parent.iter() {
        let mut main_entity = commands.entity(entity_id);
        main_entity.remove::<ZenGinModelComponentLoaded>();
        main_entity.despawn_children();
    }
}
