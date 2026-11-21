use bevy::prelude::*;

use crate::game_state::AppState;
use crate::gameplay::game_time::GameTime;
use crate::gameplay::objects::PlayerCharacter;
use crate::gui::GameLog;
use crate::input::log_capture::LogBuffer;
use crate::input::record::ReplayCommand;
use crate::zengin::{
    DebugNpc, GameNpc, NpcSpawnNameFilter, ScriptResource, SpawnNpc, SpawnPositionFilter,
    grant_test_items, seed_test_journal,
};

use super::failures::TestFailures;
use super::log_check::{check_log_exists, check_log_not_exists};
use super::screenshot::apply_replay_screenshot;
use super::state::{ActionReplay, PendingLocations};
use super::world::{
    equip_player_weapon, rotation_from_degrees, set_camera_location, teleport_player,
    toggle_player_armor,
};

/// Applies buffered replay location commands after the camera toggle, so a
/// camera switch sync point overrides the toggle's placement at the drifted
/// free camera.
pub(super) fn apply_replay_locations(
    mut pending: ResMut<PendingLocations>,
    mut player: Query<&mut Transform, (With<PlayerCharacter>, Without<Camera3d>)>,
    mut camera: Query<&mut Transform, (With<Camera3d>, Without<PlayerCharacter>)>,
) {
    for (translation, rotation) in pending.player.drain(..) {
        teleport_player(&mut player, translation, rotation);
    }
    for (translation, rotation) in pending.camera.drain(..) {
        set_camera_location(&mut camera, translation, rotation);
    }
}

/// Applies one replayed command. Returns `true` when the replay must abort
/// (an invalid screenshot path disables the replay entirely).
#[allow(clippy::cognitive_complexity)]
pub(super) fn apply_replay_command(
    replay: &mut ActionReplay,
    command: &ReplayCommand,
    gothic_script: Option<&ScriptResource>,
    player_npc: &Query<&GameNpc, With<PlayerCharacter>>,
    game_time: &mut GameTime,
    pending: &mut PendingLocations,
    commands: &mut Commands,
    game_log: &mut GameLog,
    next_state: &mut NextState<AppState>,
    npc_filter: &mut NpcSpawnNameFilter,
    spawn_filter: &mut SpawnPositionFilter,
    log_buffer: Option<&LogBuffer>,
    failures: &mut TestFailures,
) -> bool {
    match command {
        // The scheduler already resolved the delay; there is nothing to apply.
        ReplayCommand::Wait { .. } => {}
        ReplayCommand::Press { action } => {
            replay.started_buttons.insert(*action);
            replay.held_buttons.insert(*action);
        }
        ReplayCommand::PressRelease { action } => {
            replay.started_buttons.insert(*action);
            replay.held_buttons.insert(*action);
            replay.released_buttons.insert(*action);
        }
        ReplayCommand::Release { action } => {
            replay.held_buttons.remove(action);
        }
        ReplayCommand::Axis { action, value } => {
            if *value == Vec2::ZERO {
                replay.held_axes.remove(action);
            } else {
                replay.held_axes.insert(*action, *value);
            }
        }
        ReplayCommand::TeleportPlayer {
            translation,
            rotation,
        } => {
            pending.player.push((*translation, *rotation));
        }
        ReplayCommand::SetCameraLocation {
            translation,
            rotation,
        } => {
            pending.camera.push((*translation, *rotation));
        }
        ReplayCommand::ZenginSetGameTimeSpeed { speed } => {
            game_time.speed = *speed;
            info!("Replay set the in-game time speed multiplier to {speed}");
        }
        ReplayCommand::ZenginEquipWeapon { item } => {
            let Some(gothic_script) = gothic_script else {
                warn!("ZenginEquipWeapon replay command: game script is not loaded");
                return false;
            };
            equip_player_weapon(gothic_script, player_npc, item);
        }
        ReplayCommand::ZenginToggleArmor { item } => {
            let Some(gothic_script) = gothic_script else {
                warn!("ZenginToggleArmor replay command: game script is not loaded");
                return false;
            };
            toggle_player_armor(gothic_script, player_npc, item);
        }
        ReplayCommand::Screenshot { path } => {
            return apply_replay_screenshot(replay, path, commands, false);
        }
        ReplayCommand::TestScreenshot { path } => {
            return apply_replay_screenshot(replay, path, commands, true);
        }
        ReplayCommand::ZenginGiveTestItems => {
            let Some(gothic_script) = gothic_script else {
                warn!("ZenginGiveTestItems replay command: game script is not loaded");
                return false;
            };
            let Ok(player_npc) = player_npc.single() else {
                warn!("ZenginGiveTestItems replay command: player npc is not spawned");
                return false;
            };
            let player_id = player_npc.instance_id();
            let mut script = gothic_script.lock();
            let lines = grant_test_items(&mut script, player_id);
            if !lines.is_empty() {
                game_log.add(lines.join("\n"));
            }
            info!(
                "Replay granted {} test item stack(s) to player({})",
                lines.len(),
                player_id
            );
        }
        ReplayCommand::ZenginGiveTestJournal => {
            let Some(gothic_script) = gothic_script else {
                warn!("ZenginGiveTestJournal replay command: game script is not loaded");
                return false;
            };
            let mut script = gothic_script.lock();
            let count = seed_test_journal(&mut script);
            game_log.add(format!("Seeded {count} test journal topic(s)"));
            info!("Replay seeded {count} test journal topic(s)");
        }
        ReplayCommand::OpenMainMenu => {
            info!("Replay opened the main menu");
            next_state.set(AppState::MainMenu);
        }
        ReplayCommand::ZenginSpawnPlayerCharacter {
            translation,
            rotation,
        } => {
            let transform = Transform::from_translation(*translation)
                .with_rotation(rotation_from_degrees(*rotation));
            info!("Replay requested the player character spawn at {translation}");
            commands.queue(move |world: &mut World| {
                crate::zengin::request_player_spawn(world, transform);
            });
        }
        ReplayCommand::ZenginSpawnWorld { world } => {
            info!("Replay requested the zengin world spawn of {world}");
            let world_name = world.clone();
            commands.queue(move |world: &mut World| {
                crate::zengin::request_world_spawn(world, &world_name);
            });
        }
        ReplayCommand::ZenginScriptStartup { world, clean_world } => {
            let Some(gothic_script) = gothic_script else {
                panic!("ZenginScriptStartup replay command: game script is not loaded");
            };
            info!("Replay running the script entry points of {world}");
            let started = gothic_script.lock().run_world_scripts(world, *clean_world);
            assert!(
                started,
                "ZenginScriptStartup replay command: no script name for world({world})"
            );
        }
        // Includes are expanded while loading the scenario, before scheduling.
        ReplayCommand::IncludeScenario { path } => {
            warn!("IncludeScenario({path:?}) reached the replay unexpanded");
        }
        ReplayCommand::TestWorld { name } => {
            info!("Replay requested the test world {name}");
            let name = name.clone();
            commands.queue(move |world: &mut World| {
                crate::test_world::request_test_world_spawn(world, &name);
            });
        }
        ReplayCommand::ZenginFilterNpcWithName { filters } => {
            npc_filter.add(filters);
            info!("Replay npc spawn name filters: {filters:?}");
        }
        ReplayCommand::ZenginFilterSpawnWithPosition { points } => {
            spawn_filter.add(points);
            info!("Replay spawn position filters: {points:?}");
        }
        ReplayCommand::ZenginSetDebugNpc { name } => {
            info!("Replay debug npc filter: {name:?}");
            let name = name.clone();
            commands.queue(move |world: &mut World| {
                world.resource_mut::<DebugNpc>().set(&name);
            });
        }
        ReplayCommand::ZenginSpawnNpc {
            name,
            translation,
            rotation,
        } => {
            info!("Replay spawning npc {name:?} at {translation:?}");
            let Some(gothic_script) = gothic_script else {
                warn!("ZenginSpawnNpc replay command: game script is not loaded");
                return false;
            };
            let instance_id = gothic_script.lock().symbol_instance(name);
            let Some(instance_id) = instance_id else {
                warn!("ZenginSpawnNpc replay command: no npc instance named {name:?}");
                return false;
            };
            let transform = Transform {
                translation: *translation,
                rotation: rotation_from_degrees(*rotation),
                scale: Vec3::ONE,
            };
            commands.queue(move |world: &mut World| {
                let Some(mut spawns) = world.get_resource_mut::<Messages<SpawnNpc>>() else {
                    warn!("ZenginSpawnNpc replay command: npc spawn messages are not registered");
                    return;
                };
                spawns.write(SpawnNpc {
                    instance_id,
                    location_name: None,
                    transform: Some(transform),
                    exact_transform: true,
                    overlay: None,
                });
            });
        }
        ReplayCommand::ZenginSpawnMob {
            visual,
            focus_name,
            state_num,
            on_state_func,
            translation,
            rotation,
        } => {
            info!("Replay spawning mob {visual:?} at {translation:?}");
            let visual = visual.clone();
            let focus_name = focus_name.clone();
            let state_num = *state_num;
            let on_state_func = on_state_func.clone();
            let transform = Transform {
                translation: *translation,
                rotation: rotation_from_degrees(*rotation),
                scale: Vec3::ONE,
            };
            commands.queue(move |world: &mut World| {
                crate::zengin::request_mob_spawn(
                    world,
                    &visual,
                    focus_name.as_deref(),
                    state_num,
                    on_state_func.as_deref(),
                    transform,
                );
            });
        }
        ReplayCommand::ZenginSpawnChest {
            visual,
            locked,
            translation,
            rotation,
        } => {
            info!("Replay spawning chest {visual:?} at {translation:?}");
            let visual = visual.clone();
            let chest_locked = *locked;
            let transform = Transform {
                translation: *translation,
                rotation: rotation_from_degrees(*rotation),
                scale: Vec3::ONE,
            };
            commands.queue(move |world: &mut World| {
                crate::zengin::request_chest_spawn(world, &visual, chest_locked, transform);
            });
        }
        ReplayCommand::TestClearLogBuffer => {
            let Some(log_buffer) = log_buffer else {
                warn!("TestClearLogBuffer replay command ignored: log capture is not enabled");
                return false;
            };
            log_buffer.clear();
            info!("Replay cleared the captured log buffer");
        }
        ReplayCommand::TestLogExists { regex } => {
            let Some(log_buffer) = log_buffer else {
                warn!("TestLogExists replay command ignored: log capture is not enabled");
                return false;
            };
            check_log_exists(log_buffer, regex, failures);
        }
        ReplayCommand::TestLogNotExists { regex } => {
            let Some(log_buffer) = log_buffer else {
                warn!("TestLogNotExists replay command ignored: log capture is not enabled");
                return false;
            };
            check_log_not_exists(log_buffer, regex, failures);
        }
        ReplayCommand::ExitGame => {
            info!("Replay requested the game exit after the screenshots are captured");
            replay.exit_requested = true;
        }
    }
    false
}
