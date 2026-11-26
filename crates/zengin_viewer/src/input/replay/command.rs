use bevy::prelude::*;

use crate::debug::DebugNpc;
use crate::input::log_capture::LogBuffer;
use crate::input::record::ReplayCommand;
use crate::zengin::AppState;
use crate::zengin::PlayerCharacter;
use crate::zengin::gameplay::game_time::GameTime;
use crate::zengin::gui::GameLog;
use crate::zengin::save::format::SavedObjectBuilder;
use crate::zengin::{
    GameNpc, NpcSpawnNameFilter, ScriptMessages, ScriptResource, SetGameTime, SpawnFromSavedObject,
    SpawnPositionFilter, StringCanon, UseMobChange, VobPrototypes, WorldSpawnMessages,
    add_waypoint, enqueue_npc_goto, grant_test_items, load_world_prototypes, seed_test_journal,
};

use daedalus_typed::{TALENT_1H, TALENT_2H};

use super::failures::TestFailures;
use super::log_check::{check_log_exists, check_log_not_exists};
use super::screenshot::apply_replay_screenshot;
use super::state::{ActionReplay, PendingLocations};
use super::world::{
    equip_player_weapon, kill_npc, rotation_from_degrees, set_camera_location,
    set_player_attribute, set_player_fight_skill, teleport_player, toggle_player_armor,
};
use crate::test_world::TestCubeSpawn;
use dev_util::perf_debug;

/// Applies buffered replay location commands after the camera toggle, so a
/// camera switch sync point overrides the toggle's placement at the drifted
/// free camera.
pub(super) fn apply_replay_locations(
    mut pending: ResMut<PendingLocations>,
    mut player: Query<&mut Transform, (With<PlayerCharacter>, Without<Camera3d>)>,
    mut camera: Query<&mut Transform, (With<Camera3d>, Without<PlayerCharacter>)>,
) {
    let _perf = perf_debug!();
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
        ReplayCommand::ZenginSetGameTime { hour, minute } => {
            let (hour, minute) = (*hour, *minute);
            commands.queue(move |world: &mut World| {
                let Some(mut times) = world.get_resource_mut::<Messages<SetGameTime>>() else {
                    warn!(
                        "ZenginSetGameTime replay command: game time messages are not registered"
                    );
                    return;
                };
                times.write(SetGameTime { hour, minute });
            });
            info!("Replay set the in-game time to {hour:02}:{minute:02}");
        }
        ReplayCommand::ZenginEquipWeapon { item } => {
            let Some(gothic_script) = gothic_script else {
                warn!("ZenginEquipWeapon replay command: game script is not loaded");
                return false;
            };
            equip_player_weapon(gothic_script, player_npc, item);
        }
        ReplayCommand::ZenginSetPlayerOneHandSkill { skill } => {
            let Some(gothic_script) = gothic_script else {
                warn!("ZenginSetPlayerOneHandSkill replay command: game script is not loaded");
                return false;
            };
            let Ok(player_npc) = player_npc.single() else {
                warn!("ZenginSetPlayerOneHandSkill replay command: player npc is not spawned");
                return false;
            };
            let player_id = player_npc.instance_id();
            gothic_script
                .lock()
                .npc_state_mut(player_id)
                .talents
                .set_skill(TALENT_1H as i32, *skill);
            info!("Replay set the player 1H talent skill to {skill}");
        }
        ReplayCommand::ZenginSetPlayerTwoHandSkill { skill } => {
            let Some(gothic_script) = gothic_script else {
                warn!("ZenginSetPlayerTwoHandSkill replay command: game script is not loaded");
                return false;
            };
            let Ok(player_npc) = player_npc.single() else {
                warn!("ZenginSetPlayerTwoHandSkill replay command: player npc is not spawned");
                return false;
            };
            let player_id = player_npc.instance_id();
            gothic_script
                .lock()
                .npc_state_mut(player_id)
                .talents
                .set_skill(TALENT_2H as i32, *skill);
            info!("Replay set the player 2H talent skill to {skill}");
        }
        ReplayCommand::ZenginSetPlayerAttribute { attribute, value } => {
            let Some(gothic_script) = gothic_script else {
                warn!("ZenginSetPlayerAttribute replay command: game script is not loaded");
                return false;
            };
            set_player_attribute(gothic_script, player_npc, attribute, *value);
        }
        ReplayCommand::ZenginSetPlayerFightSkill { talent, value } => {
            let Some(gothic_script) = gothic_script else {
                warn!("ZenginSetPlayerFightSkill replay command: game script is not loaded");
                return false;
            };
            set_player_fight_skill(gothic_script, player_npc, *talent, *value);
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
        ReplayCommand::ZenginLoadWorldPrototypes { world } => {
            info!("Replay requested the world prototypes of {world}");
            let world_name = world.clone();
            commands.queue(move |world: &mut World| {
                load_world_prototypes(world, &world_name);
            });
        }
        ReplayCommand::ZenginSpawnVob {
            name,
            vob_id,
            visual,
            translation,
            rotation,
        } => {
            let transform = Transform::from_translation(*translation)
                .with_rotation(rotation_from_degrees(*rotation));
            let name = name.clone();
            let visual = visual.clone();
            let requested_id = *vob_id;
            info!(
                "Replay requested vob spawn {name:?}/{requested_id:?}/{visual:?} at {translation:?}"
            );
            commands.queue(move |world: &mut World| {
                let Some(prototypes) = world.get_resource::<VobPrototypes>() else {
                    warn!("ZenginSpawnVob: world prototypes are not registered");
                    return;
                };
                let id = match (requested_id, name.as_deref(), visual.as_deref()) {
                    (Some(id), _, _) => id,
                    (None, Some(name), _) => {
                        let Some(id) = prototypes.id_by_name(name) else {
                            warn!("ZenginSpawnVob: no vob named {name:?}");
                            return;
                        };
                        id
                    }
                    (None, None, Some(visual)) => {
                        let Some(id) = prototypes.id_by_visual(visual) else {
                            warn!("ZenginSpawnVob: no vob with visual {visual:?}");
                            return;
                        };
                        id
                    }
                    (None, None, None) => {
                        warn!("ZenginSpawnVob: name, vob_id or visual is required");
                        return;
                    }
                };
                if prototypes.get(id).is_none() {
                    warn!("ZenginSpawnVob: no vob prototype with id {id}");
                    return;
                }
                let Some(spawns) = world.get_resource::<WorldSpawnMessages>() else {
                    warn!("ZenginSpawnVob: world spawn messages are not registered");
                    return;
                };
                spawns.objects.send(SpawnFromSavedObject {
                    object: SavedObjectBuilder::new(transform).mob_base(id).build(),
                });
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
        ReplayCommand::PlaceCube {
            translation,
            rotation,
            size,
            color,
        } => {
            info!("Replay placing cube at {translation:?} size {size:?}");
            let cube = TestCubeSpawn {
                center: *translation,
                size: *size,
                rotation: rotation_from_degrees(*rotation),
                color: Color::srgb(color.red, color.green, color.blue),
            };
            commands.queue(move |world: &mut World| {
                crate::test_world::request_test_cube_spawn(world, cube);
            });
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
        ReplayCommand::ZenginKillNpc { name } => {
            let Some(gothic_script) = gothic_script else {
                warn!("ZenginKillNpc replay command: game script is not loaded");
                return false;
            };
            kill_npc(gothic_script, name);
        }
        ReplayCommand::ZenginSetDebugNpc { name, clear } => {
            info!("Replay debug npc filter: {name:?} clear({clear})");
            let name = name.clone();
            let clear = *clear;
            commands.queue(move |world: &mut World| {
                world.resource_mut::<DebugNpc>().add(&name, clear);
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
                let Some(spawns) = world.get_resource::<WorldSpawnMessages>() else {
                    warn!("ZenginSpawnNpc replay command: world spawn messages are not registered");
                    return;
                };
                spawns.objects.send(SpawnFromSavedObject {
                    object: SavedObjectBuilder::new(transform).npc(instance_id).build(),
                });
            });
        }
        ReplayCommand::ZenginNpcGotoWaypoint { name, waypoint } => {
            info!("Replay npc {name:?} goes to waypoint {waypoint:?}");
            let Some(gothic_script) = gothic_script else {
                warn!("ZenginNpcGotoWaypoint replay command: game script is not loaded");
                return false;
            };
            let Some(instance_id) = gothic_script.lock().symbol_instance(name) else {
                warn!("ZenginNpcGotoWaypoint replay command: no npc instance named {name:?}");
                return false;
            };
            let waypoint = StringCanon::new(waypoint);
            commands.queue(move |world: &mut World| {
                enqueue_npc_goto(world, instance_id, waypoint);
            });
        }
        ReplayCommand::ZenginAddWaypoint {
            name,
            translation,
            connect_to,
        } => {
            info!("Replay adding waypoint {name:?} at {translation:?} connected to {connect_to:?}");
            let name = name.clone();
            let connect_to = connect_to.clone();
            let translation_value = *translation;
            commands.queue(move |world: &mut World| {
                add_waypoint(world, &name, translation_value, connect_to.as_deref());
            });
        }
        ReplayCommand::ZenginNpcUseMob {
            name,
            scheme,
            state,
        } => {
            info!("Replay npc {name:?} uses mob scheme {scheme:?} state {state}");
            let Some(gothic_script) = gothic_script else {
                warn!("ZenginNpcUseMob replay command: game script is not loaded");
                return false;
            };
            let Some(instance_id) = gothic_script.lock().symbol_instance(name) else {
                warn!("ZenginNpcUseMob replay command: no npc instance named {name:?}");
                return false;
            };
            let scheme = scheme.clone();
            let state_value = *state;
            commands.queue(move |world: &mut World| {
                let Some(messages) = world.get_resource::<ScriptMessages>() else {
                    warn!("ZenginNpcUseMob replay command: script messages are not registered");
                    return;
                };
                messages.use_mob.send(UseMobChange {
                    instance_id,
                    mob_name: StringCanon::new(&scheme),
                    state: state_value,
                });
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
