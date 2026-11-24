use bevy::prelude::Vec3;
use image::{DynamicImage, Rgb, RgbImage};
use regex::Regex;
use ron::Options as RonOptions;
use ron::extensions::Extensions;

use crate::input::actions::GameplayAction;
use crate::input::record::{RecordedAction, ReplayCommand};

use super::log_check::matching_log_line;
use super::scenario::{
    ScheduledCommand, load_scenario_commands, resolve_include_path, schedule_commands,
};
use super::screenshot::{COMPARE_MAX_CHANGED_FRACTION, compare_images, diff_path};

#[test]
fn commands_run_five_frames_apart() {
    let commands = parse("[ExitGame, ExitGame]");
    assert_eq!(frames(&commands), [5, 10]);
}

#[test]
fn wait_delays_the_following_commands() {
    let commands = parse("[ExitGame, Wait(frame_dt: 20), ExitGame]");
    assert_eq!(frames(&commands), [5, 25, 30]);
}

#[test]
fn wait_is_relative_to_the_previous_command() {
    let commands = parse("[Wait(frame_dt: 7), ExitGame]");
    assert_eq!(frames(&commands), [7, 12]);
}

#[test]
fn bundled_scenarios_parse() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../misc/scenario");
    let mut files = Vec::new();
    collect_scenario_files(&dir, &mut files);
    assert!(!files.is_empty());
    for path in files {
        parse_scenario_file(&dir, &path);
    }
}

fn collect_scenario_files(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("Failed to read scenario dir {}: {error}", dir.display()));
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_scenario_files(&path, files);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("ron") {
            files.push(path);
        }
    }
}

#[test]
fn time_speed_command_parses_and_schedules() {
    let commands = parse("[ZenginSetGameTimeSpeed(speed: 2.5), Screenshot(path: \"shot.png\")]");
    assert_eq!(frames(&commands), [5, 10]);
    assert!(matches!(
        commands[0].command,
        ReplayCommand::ZenginSetGameTimeSpeed { speed } if (speed - 2.5).abs() < f32::EPSILON
    ));
}

#[test]
fn game_time_command_parses_and_schedules() {
    let commands = parse("[ZenginSetGameTime(hour: 0, minute: 30)]");
    assert_eq!(frames(&commands), [5]);
    assert!(matches!(
        commands[0].command,
        ReplayCommand::ZenginSetGameTime {
            hour: 0,
            minute: 30
        }
    ));
}

#[test]
fn world_command_parses_and_schedules() {
    let commands = parse("[TestWorld(name: \"simple\")]");
    assert_eq!(frames(&commands), [5]);
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::TestWorld { name } if name == "simple"
    ));
}

#[test]
fn script_startup_command_parses_and_schedules() {
    let commands = parse("[ZenginScriptStartup(world: \"NEWWORLD/NEWWORLD\")]");
    assert_eq!(frames(&commands), [5]);
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::ZenginScriptStartup { world, clean_world: false }
            if world == "NEWWORLD/NEWWORLD"
    ));

    let commands = parse("[ZenginScriptStartup(clean_world: true, world: \"NEWWORLD/NEWWORLD\")]");
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::ZenginScriptStartup {
            clean_world: true,
            ..
        }
    ));
}

#[test]
fn debug_npc_command_parses_and_schedules() {
    let commands = parse("[ZenginSetDebugNpc(name: \"lares\")]");
    assert_eq!(frames(&commands), [5]);
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::ZenginSetDebugNpc { name } if name == "lares"
    ));
}

#[test]
fn press_release_command_parses_and_schedules() {
    let commands = parse("[PressRelease(action: Gameplay(Jump))]");
    assert_eq!(frames(&commands), [5]);
    assert!(matches!(
        commands[0].command,
        ReplayCommand::PressRelease {
            action: RecordedAction::Gameplay(GameplayAction::Jump),
        }
    ));
}

#[test]
fn spawn_npc_command_parses_and_schedules() {
    let commands = parse(
        "[ZenginSpawnNpc(name: \"lares\", translation: (1.0, 2.0, 3.0), rotation: (0.0, 90.0, 0.0))]",
    );
    assert_eq!(frames(&commands), [5]);
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::ZenginSpawnNpc { name, translation, .. }
            if name == "lares" && *translation == Vec3::new(1.0, 2.0, 3.0)
    ));
}

#[test]
fn add_waypoint_command_parses_and_schedules() {
    let commands = parse("[ZenginAddWaypoint(name: \"wp_start\", translation: (0.0, 0.0, -20.0))]");
    assert_eq!(frames(&commands), [5]);
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::ZenginAddWaypoint { name, translation, connect_to: None }
            if name == "wp_start" && *translation == Vec3::new(0.0, 0.0, -20.0)
    ));

    let commands = parse(
        "[ZenginAddWaypoint(name: \"wp_end\", translation: (0.0, 0.0, 20.0), connect_to: Some(\"wp_start\"))]",
    );
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::ZenginAddWaypoint { connect_to: Some(target), .. } if target == "wp_start"
    ));
}

#[test]
fn npc_goto_waypoint_command_parses_and_schedules() {
    let commands = parse("[ZenginNpcGotoWaypoint(name: \"vlk_449_lares\", waypoint: \"wp_end\")]");
    assert_eq!(frames(&commands), [5]);
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::ZenginNpcGotoWaypoint { name, waypoint }
            if name == "vlk_449_lares" && waypoint == "wp_end"
    ));
}

#[test]
fn npc_use_mob_command_parses_and_schedules() {
    let commands =
        parse("[ZenginNpcUseMob(name: \"vlk_412_harad\", scheme: \"BSANVIL\", state: 1)]");
    assert_eq!(frames(&commands), [5]);
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::ZenginNpcUseMob { name, scheme, state }
            if name == "vlk_412_harad" && scheme == "BSANVIL" && *state == 1
    ));
}

#[test]
fn place_cube_command_parses_and_schedules() {
    let commands = parse(
        "[PlaceCube(translation: (0.0, 2.5, -16.0), rotation: (0.0, 0.0, 0.0), size: (6.0, 5.0, 0.5), color: (red: 0.8, green: 0.2, blue: 0.2))]",
    );
    assert_eq!(frames(&commands), [5]);
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::PlaceCube { translation, size, color, .. }
            if *translation == Vec3::new(0.0, 2.5, -16.0)
                && *size == Vec3::new(6.0, 5.0, 0.5)
                && (color.red - 0.8).abs() < f32::EPSILON
                && (color.green - 0.2).abs() < f32::EPSILON
                && (color.blue - 0.2).abs() < f32::EPSILON
    ));
}

#[test]
fn spawn_mob_command_parses_and_schedules() {
    let commands = parse(
        "[ZenginSpawnMob(visual: \"CHAIR_NW_NORMAL_01\", translation: (0.0, 0.0, -2.0), rotation: (0.0, 0.0, 0.0))]",
    );
    assert_eq!(frames(&commands), [5]);
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::ZenginSpawnMob { visual, state_num: None, focus_name: None, translation, .. }
            if visual == "CHAIR_NW_NORMAL_01"
                && *translation == Vec3::new(0.0, 0.0, -2.0)
    ));

    let commands = parse(
        "[ZenginSpawnMob(visual: \"chair\", focus_name: \"Chair\", state_num: 2, translation: (1.0, 2.0, 3.0), rotation: (0.0, 90.0, 0.0))]",
    );
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::ZenginSpawnMob { focus_name: Some(name), state_num: Some(2), .. }
            if name == "Chair"
    ));

    let commands = parse(
        "[ZenginSpawnMob(visual: \"BEDHIGH_NW_NORMAL_01\", on_state_func: \"sleepabit\", translation: (0.0, 0.0, -2.0), rotation: (0.0, 0.0, 0.0))]",
    );
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::ZenginSpawnMob { on_state_func: Some(name), .. } if name == "sleepabit"
    ));
}

#[test]
fn spawn_chest_command_parses_and_schedules() {
    let commands = parse(
        "[ZenginSpawnChest(visual: \"CHESTBIG_NW_NORMAL_OPEN\", translation: (0.0, 0.0, -2.0), rotation: (0.0, 0.0, 0.0))]",
    );
    assert_eq!(frames(&commands), [5]);
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::ZenginSpawnChest { visual, locked: false, translation, .. }
            if visual == "CHESTBIG_NW_NORMAL_OPEN"
                && *translation == Vec3::new(0.0, 0.0, -2.0)
    ));

    let commands = parse(
        "[ZenginSpawnChest(visual: \"CHESTSMALL_NW_POOR_LOCKED\", locked: true, translation: (1.0, 0.0, 0.0), rotation: (0.0, 0.0, 0.0))]",
    );
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::ZenginSpawnChest { locked: true, .. }
    ));

    let commands = parse(
        "[ZenginSpawnChest(visual: \"CHESTSMALL_NW_POOR_LOCKED\", locked: true, key: \"ITKE_KEY_01\", pick_string: \"LR\", translation: (1.0, 0.0, 0.0), rotation: (0.0, 0.0, 0.0))]",
    );
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::ZenginSpawnChest { key: Some(key), pick_string: Some(pick), .. }
            if key == "ITKE_KEY_01" && pick == "LR"
    ));
}

#[test]
fn include_paths_are_relative_to_the_scenario_root() {
    let root = std::path::Path::new("misc/scenario");
    assert_eq!(
        resolve_include_path(root, "/test_room").unwrap(),
        std::path::PathBuf::from("misc/scenario/test_room.ron")
    );
    assert_eq!(
        resolve_include_path(root, "/tests/dialog_lares.ron").unwrap(),
        std::path::PathBuf::from("misc/scenario/tests/dialog_lares.ron")
    );
    resolve_include_path(root, "test_room").unwrap_err();
    resolve_include_path(root, "misc/scenario/test_room").unwrap_err();
    resolve_include_path(root, "/").unwrap_err();
    resolve_include_path(root, "/../test_room").unwrap_err();
    resolve_include_path(root, "/./test_room").unwrap_err();
}

#[test]
fn stale_frame_fields_are_rejected() {
    RonOptions::default()
        .with_default_extension(Extensions::IMPLICIT_SOME)
        .from_str::<Vec<ReplayCommand>>("[Screenshot(frame: 2, path: \"shot.png\")]")
        .unwrap_err();
}

#[test]
fn wait_serializes_with_its_delay() {
    let command = ReplayCommand::Wait { frame_dt: 20 };
    let mut pretty_config = ron::ser::PrettyConfig::default();
    pretty_config.depth_limit = 1;
    let text = RonOptions::default()
        .with_default_extension(Extensions::IMPLICIT_SOME)
        .to_string_pretty(&vec![command], pretty_config)
        .unwrap();
    assert!(text.contains("Wait(frame_dt: 20)"), "{text}");
}

#[test]
fn identical_screenshots_match() {
    let image = test_image([20, 40, 60]);
    let report = compare_images(&image, &image.clone()).expect("same size");
    assert!(report.changed_fraction < f32::EPSILON);
    assert!(report.mean_diff < f32::EPSILON);
}

#[test]
fn large_change_is_detected() {
    let black = test_image([0, 0, 0]);
    let white = test_image([255, 255, 255]);
    let report = compare_images(&black, &white).expect("same size");
    assert!(report.changed_fraction > COMPARE_MAX_CHANGED_FRACTION);
}

#[test]
fn small_local_noise_is_accepted() {
    let image = test_image([100, 100, 100]);
    let mut noisy = image.clone().to_rgb8();
    for x in 0..12 {
        for y in 0..12 {
            noisy.put_pixel(x, y, Rgb([120, 120, 120]));
        }
    }
    let report = compare_images(&image, &DynamicImage::ImageRgb8(noisy)).expect("same size");
    assert!(report.changed_fraction <= COMPARE_MAX_CHANGED_FRACTION);
}

#[test]
fn size_mismatch_is_an_error_not_a_panic() {
    let image = test_image([0, 0, 0]);
    let other = DynamicImage::ImageRgb8(RgbImage::new(8, 8));
    assert!(compare_images(&image, &other).is_err());
}

#[test]
fn diff_path_keeps_the_screenshot_directory() {
    assert_eq!(
        diff_path(std::path::Path::new("workdir/screens/shot.png")),
        std::path::Path::new("workdir/screens/shot.diff.png")
    );
}

#[test]
fn log_commands_parse_and_schedule() {
    let commands = parse(
        r#"[
            TestClearLogBuffer,
            TestLogExists(regex: "WARN.*missing"),
            TestLogNotExists(regex: "ERROR.*missing"),
        ]"#,
    );
    assert_eq!(frames(&commands), [5, 10, 15]);
    assert!(matches!(
        &commands[1].command,
        ReplayCommand::TestLogExists { regex } if regex == "WARN.*missing"
    ));
    assert!(matches!(
        &commands[2].command,
        ReplayCommand::TestLogNotExists { regex } if regex == "ERROR.*missing"
    ));
}

#[test]
fn screenshot_command_parses_and_schedules() {
    let commands = parse("[Screenshot(path: \"plain.png\"), TestScreenshot(path: \"tested.png\")]");
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::Screenshot { path } if path == "plain.png"
    ));
    assert!(matches!(
        &commands[1].command,
        ReplayCommand::TestScreenshot { path } if path == "tested.png"
    ));
}

#[test]
fn log_exists_regex_matches_buffered_lines() {
    let lines = vec![
        "INFO zengin_viewer: loaded".to_string(),
        "WARN zengin_viewer::npc: missing texture".to_string(),
    ];
    assert!(matching_log_line(&lines, &Regex::new("missing texture").unwrap()).is_some());
    assert!(matching_log_line(&lines, &Regex::new("^ERROR").unwrap()).is_none());
}

#[test]
fn log_checks_ignore_replay_infrastructure_lines() {
    let lines = vec![
        "INFO zengin_viewer::input::replay::actions: Replay triggered command: TestLogNotExists { regex: \"missing texture\" }".to_string(),
        "WARN zengin_viewer::npc: missing texture".to_string(),
    ];
    let regex = Regex::new("missing texture").unwrap();

    assert_eq!(
        matching_log_line(&lines, &regex),
        Some("WARN zengin_viewer::npc: missing texture")
    );
    assert!(matching_log_line(&lines[..1], &regex).is_none());
}

fn parse(commands: &str) -> Vec<ScheduledCommand> {
    let commands: Vec<ReplayCommand> = RonOptions::default()
        .with_default_extension(Extensions::IMPLICIT_SOME)
        .from_str(commands)
        .expect("test ron should parse");
    schedule_commands(commands)
}

fn frames(commands: &[ScheduledCommand]) -> Vec<u64> {
    commands.iter().map(|command| command.frame).collect()
}

fn parse_scenario_file(root: &std::path::Path, path: &std::path::Path) {
    let commands = load_scenario_commands(root, path).unwrap_or_else(|error| panic!("{error}"));
    schedule_commands(commands);
}

fn test_image(color: [u8; 3]) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::from_pixel(128, 96, Rgb(color)))
}
