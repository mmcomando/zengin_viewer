use std::f32::consts::FRAC_PI_2;

use crate::game::objects::GameNpc;
use crate::toggle_visibility::{NpcVisibility, show_gizmos};
use crate::zengin::common::ZenGinModel;
use crate::zengin_resources::{MaterialHandles, ZenGinModelComponent, create_skined_mesh_data};
use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use bevy::prelude::*;

#[derive(Default)]
pub struct GameObjectSpawnEntities;

impl Plugin for GameObjectSpawnEntities {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, object_to_entities);
        app.add_systems(Update, joint_animation);
        app.add_systems(Update, draw_bones.run_if(show_gizmos));
    }
}

/// Check only entities which were not handled previously
#[derive(Component, Default)]
struct ObjectEntitiesSpawned {}

#[derive(Component, Default)]
struct NpcSpawnState {
    body_handle: Handle<ZenGinModel>,
    armor_handle: Option<Handle<ZenGinModel>>,
    body_spawned: bool,
    head_spawned: bool,
    armor_spawned: bool,
}

#[derive(Component)]
pub struct AnimatedJoint(pub isize);

#[allow(clippy::type_complexity)]
fn object_to_entities(
    models: ResMut<Assets<ZenGinModel>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut handles_map: ResMut<MaterialHandles>,
    mut query: Query<
        (Entity, &GameNpc, Option<&mut NpcSpawnState>),
        Without<ObjectEntitiesSpawned>,
    >,
    mut skinned_mesh_inverse_bindposes_assets: ResMut<Assets<SkinnedMeshInverseBindposes>>,
) {
    for (entity_id, npc_component, spawn_state) in &mut query {
        let Some(mut spawn_state) = spawn_state else {
            let spawn_state = NpcSpawnState {
                body_handle: handles_map.get_model_handle(
                    &asset_server,
                    &npc_component.body_model,
                    npc_component.hierarchy.as_deref(),
                ),
                armor_handle: npc_component.armor_model.as_ref().map(|el| {
                    handles_map.get_model_handle(
                        &asset_server,
                        el,
                        npc_component.hierarchy.as_deref(),
                    )
                }),
                body_spawned: npc_component.armor_model.is_some(),
                head_spawned: npc_component.head_model.is_none(),
                armor_spawned: npc_component.armor_model.is_none(),
            };

            let mut entity = commands.entity(entity_id);
            entity.insert(spawn_state);
            continue;
        };

        let body_load_state = asset_server.load_state(spawn_state.body_handle.id());

        if body_load_state.is_failed() {
            let mut entity = commands.entity(entity_id);
            entity.insert(ObjectEntitiesSpawned::default());
            continue;
        }

        let Some(model_data) = models.get(&spawn_state.body_handle) else {
            continue;
        };

        let armor_data = spawn_state
            .armor_handle
            .as_ref()
            .and_then(|el| models.get(el));
        if spawn_state.armor_handle.is_some() && armor_data.is_none() {
            continue;
        }

        {
            let mut entity = commands.entity(entity_id);
            entity.insert(ObjectEntitiesSpawned::default());
        }

        let bones_data = create_skined_mesh_data(
            &mut commands,
            &mut skinned_mesh_inverse_bindposes_assets,
            entity_id,
            model_data,
        );
        let head_node_index = model_data
            .node_names
            .iter()
            .position(|el| el == "BIP01 HEAD");

        if let Some(head_model) = &npc_component.head_model {
            let tr = Transform::IDENTITY;

            let head_entity_id = if let Some(bones_data) = &bones_data
                && let Some(head_node_index) = head_node_index
            {
                bones_data.joints[head_node_index]
            } else {
                entity_id
            };

            let mut entity = commands.entity(head_entity_id);

            entity.with_child((
                Visibility::default(),
                ZenGinModelComponent {
                    model_handle: handles_map.get_model_handle(&asset_server, head_model, None),
                    override_texture: npc_component.head_texture.clone(),
                    ..default()
                },
                tr,
            ));
            spawn_state.head_spawned = true;
        }
        if !spawn_state.body_spawned {
            spawn_state.body_spawned = true;
            let tr = Transform::IDENTITY;

            let mut entity = commands.entity(entity_id);
            entity.with_child((
                Visibility::default(),
                ZenGinModelComponent {
                    model_handle: spawn_state.body_handle.clone(),
                    override_texture: npc_component.body_texture.clone(),
                    bones_data: bones_data.clone(),
                    ..default()
                },
                tr,
            ));
        }
        if let Some(armor_model) = &npc_component.armor_model {
            let armor_data = armor_data.unwrap();

            let bones_data = create_skined_mesh_data(
                &mut commands,
                &mut skinned_mesh_inverse_bindposes_assets,
                entity_id,
                armor_data,
            );

            let tr = Transform::IDENTITY;

            let mut entity = commands.entity(entity_id);
            entity.with_child((
                Visibility::default(),
                ZenGinModelComponent {
                    model_handle: handles_map.get_model_handle(
                        &asset_server,
                        armor_model,
                        npc_component.hierarchy.as_deref(),
                    ),
                    bones_data,
                    ..default()
                },
                tr,
                NpcVisibility::default(),
            ));
            spawn_state.armor_spawned = true;
        }
    }
}

/// Animate the joint marked with [`AnimatedJoint`] component.
fn joint_animation(time: Res<Time>, mut query: Query<(&mut Transform, &AnimatedJoint)>) {
    for (mut transform, animated_joint) in &mut query {
        match animated_joint.0 {
            -5 => {
                transform.rotation =
                    Quat::from_rotation_x(FRAC_PI_2 * ops::sin(time.elapsed_secs()));
            }
            -4 => {
                transform.rotation =
                    Quat::from_rotation_y(FRAC_PI_2 * ops::sin(time.elapsed_secs()));
            }
            -3 => {
                transform.rotation =
                    Quat::from_rotation_z(FRAC_PI_2 * ops::sin(time.elapsed_secs()));
            }
            -2 => {
                transform.scale.x = ops::sin(time.elapsed_secs()) + 1.0;
            }
            -1 => {
                transform.scale.y = ops::sin(time.elapsed_secs()) + 1.0;
            }
            1 => {
                transform.translation.y = ops::sin(time.elapsed_secs());
                transform.translation.z = ops::cos(time.elapsed_secs());
            }
            2 => {
                transform.translation.x = ops::sin(time.elapsed_secs());
            }
            3 => {
                transform.translation.y = ops::sin(time.elapsed_secs());
                transform.scale.x = ops::sin(time.elapsed_secs()) + 1.0;
            }
            4 => {
                transform.translation.x = 0.5 * ops::sin(time.elapsed_secs());
                transform.translation.y = ops::cos(time.elapsed_secs());
            }
            _ => (),
        }
    }
}

fn draw_bones(
    mut gizmos: Gizmos,
    bones: Query<(Entity, &GlobalTransform, Option<&ChildOf>), With<AnimatedJoint>>,
    transforms: Query<&GlobalTransform>,
) {
    for (_bone_entity, bone_transform, parent) in bones.iter() {
        if let Some(parent) = parent
            && let Ok(parent_transform) = transforms.get(parent.0)
        {
            // Draw a line from parent bone to current bone
            gizmos.line(
                parent_transform.translation(),
                bone_transform.translation(),
                Color::srgb(0.0, 1.0, 0.0), // Green bones
            );

            // Optional: Draw the bone itself (a small sphere)
            // gizmos.sphere(
            //     bone_transform.translation(),
            //     // Quat::IDENTITY,
            //     0.02,
            //     Color::srgb(1.0, 0.0, 0.0), // Red joints
            // );
        }
    }
}
