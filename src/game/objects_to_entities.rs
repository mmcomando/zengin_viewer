use crate::game::objects::GameNpc;
use crate::toggle_visibility::{NpcVisibility, show_gizmos};
use crate::zengin::common::ZenGinModel;
use crate::zengin_resources::{MaterialHandles, ZenGinModelComponent};
use bevy::prelude::*;

#[derive(Default)]
pub struct GameObjectSpawnEntities;

impl Plugin for GameObjectSpawnEntities {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, object_to_entities);
        app.add_systems(Update, show_bones.run_if(show_gizmos));
    }
}

/// Check only entities which were not handled previously
#[derive(Component, Default)]
struct ObjectEntitiesSpawned {}

#[derive(Component, Default)]
struct NpcSpawnState {
    body_handle: Handle<ZenGinModel>,
    body_spawned: bool,
    head_spawned: bool,
    armor_spawned: bool,
}

impl NpcSpawnState {
    fn finished_spawninig(&self) -> bool {
        return self.body_spawned && self.head_spawned && self.armor_spawned;
    }
}

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
) {
    for (entity_id, npc_component, spawn_state) in &mut query {
        let mut entity = commands.entity(entity_id);

        let Some(mut spawn_state) = spawn_state else {
            let spawn_state = NpcSpawnState {
                body_handle: handles_map.get_model_handle(&asset_server, &npc_component.body_model),
                body_spawned: npc_component.armor_model.is_some(),
                head_spawned: npc_component.head_model.is_none(),
                armor_spawned: npc_component.armor_model.is_none(),
            };

            entity.insert(spawn_state);
            continue;
        };

        let body_load_state = asset_server.load_state(spawn_state.body_handle.id());

        if body_load_state.is_failed() {
            entity.insert(ObjectEntitiesSpawned::default());
            continue;
        }

        let Some(model_data) = models.get(&spawn_state.body_handle) else {
            continue;
        };

        if !spawn_state.body_spawned {
            spawn_state.body_spawned = true;
            let tr = Transform::IDENTITY;
            let tr = npc_component.tr * tr;
            entity.with_child((
                Visibility::default(),
                ZenGinModelComponent {
                    model_handle: spawn_state.body_handle.clone(),
                    override_texture: npc_component.body_texture.clone(),
                    ..default()
                },
                tr,
            ));

            if spawn_state.finished_spawninig() {
                entity.insert(ObjectEntitiesSpawned::default());
            }
        }

        if let Some(head_model) = &npc_component.head_model {
            let tr = if let Some(tr) = model_data.nodes_tr.get("BIP01 HEAD") {
                *tr
            } else {
                Transform::IDENTITY
            };
            let tr = npc_component.tr * tr;
            entity.with_child((
                Visibility::default(),
                ZenGinModelComponent {
                    model_handle: handles_map.get_model_handle(&asset_server, head_model),
                    override_texture: npc_component.head_texture.clone(),
                    ..default()
                },
                tr,
            ));
            spawn_state.head_spawned = true;
            if spawn_state.finished_spawninig() {
                entity.insert(ObjectEntitiesSpawned::default());
            }
        }
        if let Some(armor_model) = &npc_component.armor_model {
            warn_once!("Armor detailed placing requires hardcoding");
            let tr = Transform::from_translation(Vec3 {
                x: -0.02,
                y: 0.018,
                z: 0.15,
            });
            let tr = npc_component.tr * tr;
            entity.with_child((
                Visibility::default(),
                ZenGinModelComponent {
                    model_handle: handles_map.get_model_handle(&asset_server, armor_model),
                    ..default()
                },
                tr,
                NpcVisibility::default(),
            ));
            spawn_state.armor_spawned = true;
            if spawn_state.finished_spawninig() {
                entity.insert(ObjectEntitiesSpawned::default());
            }
        }
    }
}

fn show_bones(
    models: ResMut<Assets<ZenGinModel>>,
    query: Query<(&Transform, &NpcSpawnState), With<ObjectEntitiesSpawned>>,
    mut gizmos: Gizmos,
) {
    for (entity_tr, spawn_state) in &query {
        let Some(model_data) = models.get(&spawn_state.body_handle) else {
            continue;
        };

        gizmos.axes(*entity_tr, 1.0);

        for (key, tr) in &model_data.nodes_tr {
            let show = key == "BIP01 HEAD" || key == "BIP01 PELVIS";
            if !show {
                continue;
            }
            let tr = *entity_tr * *tr;
            gizmos.axes(tr, 0.2);
        }
    }
}
