//! Quick save/load triggers, save collection, and the destructive load.

use std::collections::HashSet;
use std::path::PathBuf;

use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::game_state::{AppState, world_is_loaded};
use crate::gameplay::camera::CameraResource;
use crate::gameplay::character::PlayerPhysicsBodyMarker;
use crate::gameplay::game_time::GameTime;
use crate::gameplay::objects::PlayerCharacter;
use crate::gui::{GameLogEntry, InventoryUiState, JournalUiState, StatsUiState};
use crate::input::{SharedAction, SharedInputMarker};
use crate::save::format::{
    SAVE_FORMAT_VERSION, SaveGame, SavedChest, SavedItemEntity, SavedNpcEntity,
};
use crate::save::{
    current_script_fingerprint, newest_save, read_save, timestamped_save_path, write_save,
};
use crate::world_entity::WorldEntityMarker;
use crate::zengin::request_initial_world;
use crate::zengin::{
    CurrentWorld, DialogUiState, GameChest, GameItem, GameNpc, InstanceIndex, NpcOverlay,
    ScriptResource, Selection, SpawnWorld,
};

/// Requests writing a new timestamped save.
#[derive(Message)]
pub struct QuickSave;

/// Requests loading the newest timestamped save.
#[derive(Message)]
pub struct QuickLoad;

/// A save file selected in the menu. It is parsed and validated before the
/// state changes, so a corrupt or incompatible save leaves the menu open.
#[derive(Message)]
pub struct LoadSaveRequest {
    pub(crate) path: PathBuf,
}

/// Parsed save waiting to be applied after entering [`AppState::LoadScreen`].
#[derive(Resource, Default)]
pub(crate) struct PendingLoad(pub(crate) Option<Box<SaveGame>>);

/// Wires the quicksave trigger actions, the save system and the exclusive load.
pub struct QuickSavePlugin;

impl Plugin for QuickSavePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<QuickSave>();
        app.add_message::<QuickLoad>();
        app.add_message::<LoadSaveRequest>();
        app.init_resource::<PendingLoad>();
        app.add_systems(Update, handle_load_save_request);
        app.add_systems(
            OnEnter(AppState::LoadScreen),
            apply_pending_load.after(request_initial_world),
        );
        app.add_systems(
            Update,
            handle_quick_save_load_input.run_if(in_state(AppState::InGame)),
        );
        // Saving is also reachable from the main menu, so it is gated on a
        // loaded world instead of the `InGame` state.
        app.add_systems(Last, quick_save.run_if(world_is_loaded));
        app.add_systems(
            First,
            quick_load
                .run_if(in_state(AppState::InGame))
                .run_if(on_message::<QuickLoad>),
        );
    }
}

fn handle_quick_save_load_input(
    action_state: Single<&ActionState<SharedAction>, With<SharedInputMarker>>,
    mut quick_save: MessageWriter<QuickSave>,
    mut quick_load: MessageWriter<QuickLoad>,
) {
    if action_state.just_pressed(&SharedAction::QuickSave) {
        quick_save.write(QuickSave);
    }
    if action_state.just_pressed(&SharedAction::QuickLoad) {
        quick_load.write(QuickLoad);
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::cognitive_complexity)]
fn quick_save(
    mut requests: MessageReader<QuickSave>,
    mut game_log: MessageWriter<GameLogEntry>,
    scripts: Res<ScriptResource>,
    current_world: Res<CurrentWorld>,
    game_time: Res<GameTime>,
    player: Query<&Transform, With<PlayerPhysicsBodyMarker>>,
    npcs: Query<(&GameNpc, &Transform, Option<&NpcOverlay>), Without<PlayerCharacter>>,
    items: Query<(&GameItem, &Transform)>,
    chests: Query<&GameChest>,
) {
    if requests.read().count() == 0 {
        return;
    }
    let world_name = current_world.0.clone();
    if world_name.is_empty() {
        error!("Save requested before a world was spawned");
        return;
    }
    let Ok(player_transform) = player.single() else {
        error!("Save requested without a player entity");
        return;
    };

    let (script_fingerprint, mut script) = {
        let script = scripts.lock();
        (script.script_fingerprint(), script.snapshot_save())
    };

    // A saved runtime without a live entity is a dead npc; `Npc_IsDead` is
    // derived from the live position, so clear it for those instances.
    let live_npcs: HashSet<i32> = npcs
        .iter()
        .map(|(npc, _, _)| npc.instance_id().value())
        .collect();
    for (instance, runtime) in &mut script.npc_runtimes {
        if live_npcs.contains(&instance.value()) {
            continue;
        }
        runtime.live.position = None;
        runtime.live.look_at = None;
        runtime.live.target = None;
    }

    let live_npcs: Vec<SavedNpcEntity> = npcs
        .iter()
        .map(|(npc, transform, overlay)| SavedNpcEntity {
            instance_id: npc.instance_id(),
            transform: *transform,
            overlay: overlay.and_then(|overlay| overlay.0.clone()),
        })
        .collect();
    let live_items: Vec<SavedItemEntity> = items
        .iter()
        .map(|(item, transform)| SavedItemEntity {
            instance_id: item.instance_id(),
            transform: *transform,
        })
        .collect();
    let chests: Vec<SavedChest> = chests
        .iter()
        .map(|chest| SavedChest {
            index: chest.index,
            items: chest.items.clone(),
        })
        .collect();

    let save = SaveGame {
        format_version: SAVE_FORMAT_VERSION,
        script_fingerprint,
        world_name,
        game_time: *game_time,
        player_transform: *player_transform,
        script,
        live_npcs,
        live_items,
        chests,
    };

    let path = match timestamped_save_path() {
        Ok(path) => path,
        Err(error) => {
            error!("Save failed: {error}");
            return;
        }
    };
    match write_save(&save, &path) {
        Ok(()) => {
            let size = std::fs::metadata(&path).map_or(0, |meta| meta.len());
            let name = crate::save::display_name(&path);
            info!("Save written to {} ({size} bytes)", path.display());
            game_log.write(GameLogEntry {
                text: format!("Game saved: {name} ({size} bytes)"),
            });
        }
        Err(error) => error!("Save failed: {error}"),
    }
}

fn quick_load(world: &mut World) {
    let script_fingerprint = world
        .resource::<ScriptResource>()
        .lock()
        .script_fingerprint();
    let Some(path) = newest_save() else {
        report_load_error(world, "no saved games found");
        return;
    };
    let save = match read_save(&path) {
        Ok(save) => save,
        Err(error) => {
            report_load_error(world, &error);
            return;
        }
    };
    if let Err(error) = save.validate(script_fingerprint) {
        report_load_error(world, &error);
        return;
    }

    world
        .resource_mut::<NextState<AppState>>()
        .set(AppState::LoadScreen);
    perform_load(world, save);
}

/// Validates a save selected in the main menu and queues it for the next
/// `LoadScreen` entry. Runs while the menu is still open so a failure does not
/// leave the menu or destroy the current world.
fn handle_load_save_request(
    mut requests: MessageReader<LoadSaveRequest>,
    mut pending: ResMut<PendingLoad>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for request in requests.read() {
        let prepared = current_script_fingerprint().and_then(|fingerprint| {
            let save = read_save(&request.path)?;
            save.validate(fingerprint)?;
            Ok(save)
        });
        match prepared {
            Ok(save) => {
                info!("Load requested from menu: {}", request.path.display());
                pending.0 = Some(Box::new(save));
                next_state.set(AppState::LoadScreen);
            }
            Err(error) => error!("Load failed for {}: {error}", request.path.display()),
        }
    }
}

fn apply_pending_load(world: &mut World) {
    let Some(save) = world.resource_mut::<PendingLoad>().0.take() else {
        return;
    };
    info!("Applying pending load of world({})", save.world_name);
    perform_load(world, *save);
}

/// Destructive part of a load: closes UIs and replaces the current world with
/// the saved one. The caller is responsible for entering `AppState::LoadScreen`.
fn perform_load(world: &mut World, save: SaveGame) {
    world.resource_mut::<InventoryUiState>().close();
    world.resource_mut::<JournalUiState>().close();
    world.resource_mut::<StatsUiState>().close();
    world.resource_mut::<DialogUiState>().close();
    *world.resource_mut::<Selection>() = Selection::default();
    world
        .resource_mut::<CameraResource>()
        .set_third_person_camera();
    *world.resource_mut::<GameTime>() = save.game_time;

    let entities: Vec<Entity> = world
        .query_filtered::<Entity, With<WorldEntityMarker>>()
        .iter(world)
        .collect();
    for entity in entities {
        world.despawn(entity);
    }
    world.resource_mut::<InstanceIndex>().clear();

    info!("Load: spawning world({})", save.world_name);
    let world_name = save.world_name.clone();
    let player_transform = save.player_transform;
    world.write_message(SpawnWorld {
        world_name,
        clean_spawn: false,
        player_transform: Some(player_transform),
        restore: Some(Box::new(save)),
    });
    world.write_message(GameLogEntry {
        text: "Loaded save".to_string(),
    });
}

fn report_load_error(world: &mut World, error: &str) {
    error!("Load failed: {error}");
    world.write_message(GameLogEntry {
        text: format!("Load failed: {error}"),
    });
}
