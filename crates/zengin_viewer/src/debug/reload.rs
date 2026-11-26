//! Debug-only entity round-trip: serialize the keepable world objects, empty the
//! world, then respawn the serialized objects after a short delay.

use bevy::prelude::*;

use dev_util::perf_debug;
use zengin_lib::AppState;
use zengin_lib::PlayerCharacter;
use zengin_lib::SavedObject;
use zengin_lib::StaticMesh;
use zengin_lib::WorldEntityMarker;
use zengin_lib::{
    GameChest, GameItem, GameNpc, MobBase, NpcOverlay, SpawnFromSavedObject, WorldSpawnMessages,
};

/// Frames the world stays empty before the serialized objects are restored.
const RESTORE_DELAY_FRAMES: u32 = 10;

/// Debug UI request: serialize every saveable object, despawn every
/// [`WorldEntityMarker`] entity, then spawn the serialized objects back after
/// [`RESTORE_DELAY_FRAMES`].
#[derive(Message)]
pub(crate) struct ReloadWorldEntities;

#[derive(Resource, Default)]
struct ReloadState {
    requested: bool,
    frames_left: u32,
    objects: Vec<SavedObject>,
}

pub(crate) struct DebugReloadPlugin;

impl Plugin for DebugReloadPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ReloadWorldEntities>();
        app.init_resource::<ReloadState>();
        app.add_systems(
            Update,
            (request_reload, reload_entities)
                .chain()
                .run_if(in_state(AppState::InGame)),
        );
    }
}

fn request_reload(
    mut requests: MessageReader<ReloadWorldEntities>,
    mut state: ResMut<ReloadState>,
) {
    if requests.read().count() == 0 {
        return;
    }
    state.requested = true;
}

enum ReloadStep {
    Idle,
    Collect,
    Wait,
    Restore,
}

/// Exclusive system: the collect/despawn and the restore pass both need full
/// world access, and the restore is delayed by [`RESTORE_DELAY_FRAMES`].
fn reload_entities(world: &mut World) {
    let _perf = perf_debug!();
    match next_step(world) {
        ReloadStep::Idle | ReloadStep::Wait => {}
        ReloadStep::Collect => collect_and_clear(world),
        ReloadStep::Restore => restore_objects(world),
    }
}

fn next_step(world: &mut World) -> ReloadStep {
    let mut state = world.resource_mut::<ReloadState>();
    if state.requested {
        state.requested = false;
        state.frames_left = RESTORE_DELAY_FRAMES;
        return ReloadStep::Collect;
    }
    if state.frames_left == 0 {
        return ReloadStep::Idle;
    }
    state.frames_left -= 1;
    if state.frames_left == 0 {
        ReloadStep::Restore
    } else {
        ReloadStep::Wait
    }
}

fn collect_and_clear(world: &mut World) {
    let objects = collect_saved_objects(world);
    let count = objects.len();
    despawn_world_entities(world);
    world.resource_mut::<ReloadState>().objects = objects;
    info!("Debug reload: serialized {count} objects, world entities despawned");
}

fn restore_objects(world: &mut World) {
    let objects = std::mem::take(&mut world.resource_mut::<ReloadState>().objects);
    let count = objects.len();
    let Some(spawns) = world.get_resource::<WorldSpawnMessages>() else {
        warn!("Debug reload: world spawn messages are not registered");
        return;
    };
    for object in objects {
        spawns.objects.send(SpawnFromSavedObject { object });
    }
    info!("Debug reload: restored {count} objects");
}

#[allow(clippy::type_complexity)]
fn collect_saved_objects(world: &mut World) -> Vec<SavedObject> {
    let mut query = world.query::<(
        &Transform,
        Option<&PlayerCharacter>,
        Option<&GameNpc>,
        Option<&NpcOverlay>,
        Option<&GameItem>,
        Option<&GameChest>,
        Option<&MobBase>,
        Option<&StaticMesh>,
    )>();
    query
        .iter(world)
        .filter_map(
            |(transform, player, npc, overlay, item, chest, base, static_mesh)| {
                SavedObject::from_components(
                    transform,
                    player,
                    npc,
                    overlay,
                    item,
                    chest,
                    base,
                    static_mesh,
                )
            },
        )
        .collect()
}

fn despawn_world_entities(world: &mut World) {
    let entities: Vec<Entity> = {
        let mut query = world.query_filtered::<Entity, With<WorldEntityMarker>>();
        query.iter(world).collect()
    };
    for entity in entities {
        world.despawn(entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn_chest(world: &mut World) {
        world.spawn((
            Transform::default(),
            GameChest {
                locked: true,
                key: String::new(),
                pick_string: String::new(),
                cracked: false,
                items: Vec::new(),
            },
            MobBase {
                vob_id: 7,
                ..default()
            },
        ));
    }

    #[test]
    fn collects_saveable_objects_and_despawns_world_entities() {
        let mut world = World::new();
        world.spawn((Transform::default(), PlayerCharacter));
        spawn_chest(&mut world);
        world.spawn((
            Transform::default(),
            StaticMesh,
            MobBase {
                vob_id: 42,
                ..default()
            },
        ));
        // Marker-only entity: despawned, but it has no SavedObject payload.
        world.spawn(WorldEntityMarker);

        let objects = collect_saved_objects(&mut world);
        assert_eq!(objects.len(), 3);
        assert!(objects.iter().any(|object| object.player.is_some()));
        assert!(objects.iter().any(|object| {
            object.chest.is_some()
                && object
                    .mob_base
                    .as_ref()
                    .is_some_and(|base| base.vob_id == 7)
        }));
        assert!(objects.iter().any(|object| {
            object.static_object.is_some()
                && object
                    .mob_base
                    .as_ref()
                    .is_some_and(|base| base.vob_id == 42)
        }));

        despawn_world_entities(&mut world);
        let mut query = world.query_filtered::<Entity, With<WorldEntityMarker>>();
        assert_eq!(query.iter(&world).count(), 0);
    }

    #[test]
    fn restore_waits_ten_frames() {
        let mut world = World::new();
        world.insert_resource(ReloadState {
            requested: true,
            ..default()
        });

        assert!(matches!(next_step(&mut world), ReloadStep::Collect));
        for _ in 0..9 {
            assert!(matches!(next_step(&mut world), ReloadStep::Wait));
        }
        assert!(matches!(next_step(&mut world), ReloadStep::Restore));
        assert!(matches!(next_step(&mut world), ReloadStep::Idle));
    }
}
