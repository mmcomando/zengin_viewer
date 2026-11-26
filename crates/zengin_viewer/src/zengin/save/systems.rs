//! Quick save/load triggers, save collection, and the destructive load.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use image::ImageFormat;
use image::imageops::FilterType;
use leafwing_input_manager::prelude::ActionState;

use crate::game_state::{AppState, world_is_loaded};
use crate::gameplay::game_time::{GameTime, PlayTime};
use crate::input::{Options, SharedAction, SharedInputMarker};
use crate::zengin::PlayerCharacter;
use crate::zengin::StaticMesh;
use crate::zengin::gui::GameLogEntry;
use crate::zengin::save::format::{
    SAVE_FORMAT_VERSION, SCREENSHOT_FILE, SaveGame, SaveMetadata, SavedObject,
};
use crate::zengin::save::{
    current_script_fingerprint, new_save_dir, newest_save, read_save, write_save,
};
use crate::zengin::{
    CurrentWorld, GameChest, GameItem, GameNpc, LoadingWorld, MobBase, NpcOverlay, ScriptResource,
    begin_world_load,
};
use dev_util::perf_debug;

/// Size of the saved thumbnail. The load screen slot is 4:3, so a full window
/// screenshot is stretched into this size.
const THUMBNAIL_WIDTH: u32 = 512;
const THUMBNAIL_HEIGHT: u32 = 384;

/// Requests writing a new timestamped save.
#[derive(Message)]
pub struct QuickSave;

/// Requests loading the newest timestamped save.
#[derive(Message)]
pub struct QuickLoad;

/// A save directory selected in the menu. It is parsed and validated before the
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
        app.add_systems(OnEnter(AppState::LoadScreen), apply_pending_load);
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
    let _perf = perf_debug!();
    if action_state.just_pressed(&SharedAction::QuickSave) {
        quick_save.write(QuickSave);
    }
    if action_state.just_pressed(&SharedAction::QuickLoad) {
        quick_load.write(QuickLoad);
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::cognitive_complexity)]
#[allow(clippy::type_complexity)]
fn quick_save(
    mut commands: Commands,
    mut requests: MessageReader<QuickSave>,
    mut game_log: MessageWriter<GameLogEntry>,
    scripts: Res<ScriptResource>,
    current_world: Res<CurrentWorld>,
    game_time: Res<GameTime>,
    play_time: Res<PlayTime>,
    options: Res<Options>,
    entities: Query<(
        &Transform,
        Option<&PlayerCharacter>,
        Option<&GameNpc>,
        Option<&NpcOverlay>,
        Option<&GameItem>,
        Option<&GameChest>,
        Option<&MobBase>,
        Option<&StaticMesh>,
    )>,
) {
    let _perf = perf_debug!();
    if requests.read().count() == 0 {
        return;
    }
    let world_name = current_world.0.clone();
    if world_name.is_empty() {
        error!("Save requested before a world was spawned");
        return;
    }

    // Every entity is saved by the components it carries, never by its kind.
    // Transform-only entities (world meshes, GUI, camera) produce no object.
    let objects: Vec<SavedObject> = entities
        .iter()
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
        .collect();
    if !objects.iter().any(|object| object.player.is_some()) {
        error!("Save requested without a player entity");
        return;
    }

    let (script_fingerprint, mut script) = {
        let script = scripts.lock();
        (script.script_fingerprint(), script.snapshot_save())
    };

    // A saved runtime without a live entity is a dead npc, mark it so
    // `Npc_IsDead` stays true after loading.
    let live_npcs: HashSet<i32> = objects
        .iter()
        .filter(|object| object.player.is_none())
        .filter_map(|object| object.npc.as_ref())
        .map(|npc| npc.instance_id.value())
        .collect();
    for (instance, runtime) in &mut script.npc_runtimes {
        if live_npcs.contains(&instance.value()) {
            continue;
        }
        runtime.live.dead = true;
        runtime.live.position = None;
        runtime.live.look_at = None;
        runtime.live.target = None;
    }

    let (path, saved_at_unix) = match new_save_dir() {
        Ok(dir) => dir,
        Err(error) => {
            error!("Save failed: {error}");
            return;
        }
    };
    let save = SaveGame {
        metadata: SaveMetadata {
            format_version: SAVE_FORMAT_VERSION,
            script_fingerprint,
            world_name,
            saved_at_unix,
            game_time: *game_time,
            play_time_seconds: play_time.seconds,
            screenshot: (!options.renderless()).then(|| SCREENSHOT_FILE.to_string()),
        },
        script,
        objects,
    };

    match write_save(&save, &path) {
        Ok(()) => {
            let size = directory_size(&path);
            let name = crate::zengin::save::display_name(&path);
            info!("Save written to {} ({size} bytes)", path.display());
            game_log.write(GameLogEntry {
                text: format!("Game saved: {name} ({size} bytes)"),
            });
            if !options.renderless() {
                // TODO: capture the game view instead of the whole window; a save
                // requested from the main menu currently includes the menu in the
                // thumbnail. Hide the UI for one frame or render to an offscreen
                // camera once this becomes noticeable.
                commands
                    .spawn(Screenshot::primary_window())
                    .observe(save_screenshot(path.join(SCREENSHOT_FILE)));
            }
        }
        Err(error) => error!("Save failed: {error}"),
    }
}

/// Resizes a captured window screenshot to the load-screen thumbnail size and
/// writes it into the save directory.
fn save_screenshot(path: PathBuf) -> impl FnMut(On<ScreenshotCaptured>) {
    move |captured: On<ScreenshotCaptured>| {
        let _perf = dev_util::perf_debug::PerfDebug::new("save::systems::save_screenshot");
        let result = captured
            .image
            .clone()
            .try_into_dynamic()
            .map_err(|error| error.to_string())
            .and_then(|image| {
                image
                    .resize_exact(THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT, FilterType::Triangle)
                    .save_with_format(&path, ImageFormat::Png)
                    .map_err(|error| error.to_string())
            });
        match result {
            Ok(()) => info!("Saved thumbnail to {}", path.display()),
            Err(error) => error!("Failed to save thumbnail {}: {error}", path.display()),
        }
    }
}

fn directory_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.metadata().ok())
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.len())
        .sum()
}

fn quick_load(world: &mut World) {
    let _perf = perf_debug!();
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

    perform_load(world, save);
}

/// Validates a save selected in the main menu and queues it for the next
/// `LoadScreen` entry. Runs while the menu is still open so a failure does not
/// leave the menu or destroy the current world.
fn handle_load_save_request(
    mut requests: MessageReader<LoadSaveRequest>,
    mut pending: ResMut<PendingLoad>,
    mut loading_world: ResMut<LoadingWorld>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let _perf = perf_debug!();
    for request in requests.read() {
        let prepared = current_script_fingerprint().and_then(|fingerprint| {
            let save = read_save(&request.path)?;
            save.validate(fingerprint)?;
            Ok(save)
        });
        match prepared {
            Ok(save) => {
                info!("Load requested from menu: {}", request.path.display());
                loading_world.0.clone_from(&save.metadata.world_name);
                pending.0 = Some(Box::new(save));
                next_state.set(AppState::LoadScreen);
            }
            Err(error) => error!("Load failed for {}: {error}", request.path.display()),
        }
    }
}

fn apply_pending_load(world: &mut World) {
    let _perf = perf_debug!();
    let Some(save) = world.resource_mut::<PendingLoad>().0.take() else {
        return;
    };
    info!(
        "Applying pending load of world({})",
        save.metadata.world_name
    );
    perform_load(world, *save);
}

/// Replaces the current world with the saved one. The save was validated before
/// this point.
fn perform_load(world: &mut World, save: SaveGame) {
    info!("Load: spawning world({})", save.metadata.world_name);
    let world_name = save.metadata.world_name.clone();
    let player_transform = save.player_transform();
    begin_world_load(world, &world_name, player_transform, Some(Box::new(save)));
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
