use bevy::prelude::Vec3;
use image::{DynamicImage, Rgb, RgbImage};
use regex::Regex;
use ron::Options as RonOptions;
use ron::extensions::Extensions;

use crate::input::actions::GameplayAction;
use crate::input::record::{RecordedAction, ReplayCommand};

use super::log_check::matching_log_line;
use super::scenario::{ScheduledCommand, schedule_commands};
use super::screenshot::{COMPARE_MAX_CHANGED_FRACTION, compare_images, diff_path};

#[test]
fn frame_dt_is_relative_to_previous_command() {
    let commands =
        parse("[ExitGame(frame: 10), ExitGame(frame_dt: 5), ExitGame(frame_dt: 2)]").unwrap();
    assert_eq!(frames(&commands), [10, 15, 17]);
}

#[test]
fn commands_are_sorted_by_resolved_frame() {
    let commands =
        parse("[ExitGame(frame: 10), ExitGame(frame: 4), ExitGame(frame_dt: 1)]").unwrap();
    assert_eq!(frames(&commands), [4, 5, 10]);
}

#[test]
fn missing_timing_defaults_to_frame_dt_5() {
    let commands = parse("[ExitGame(frame: 10), ZenginEquipWeapon(item: \"sword\")]").unwrap();
    assert_eq!(frames(&commands), [10, 15]);
}

#[test]
fn command_with_both_frames_is_rejected() {
    assert!(parse("[ZenginEquipWeapon(frame: 1, frame_dt: 1, item: \"sword\")]").is_err());
}

#[test]
fn first_command_frame_dt_is_relative_to_zero() {
    let commands = parse("[ZenginEquipWeapon(frame_dt: 2, item: \"sword\")]").unwrap();
    assert_eq!(frames(&commands), [2]);
}

#[test]
fn bundled_scenarios_parse() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../misc/scenario");
    let mut files = Vec::new();
    collect_scenario_files(&dir, &mut files);
    assert!(!files.is_empty());
    for path in files {
        parse_scenario_file(&path);
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
    let commands =
        parse("[ZenginSetGameTimeSpeed(frame: 4, speed: 2.5), Screenshot(path: \"shot.png\")]")
            .unwrap();
    assert_eq!(frames(&commands), [4, 9]);
    assert!(matches!(
        commands[0].command,
        ReplayCommand::ZenginSetGameTimeSpeed { speed, .. } if speed == 2.5
    ));
}

#[test]
fn world_command_parses_and_schedules() {
    let commands = parse("[TestWorld(frame: 3, name: \"simple\")]").unwrap();
    assert_eq!(frames(&commands), [3]);
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::TestWorld { name, .. } if name == "simple"
    ));
}

#[test]
fn script_startup_command_parses_and_schedules() {
    let commands = parse("[ZenginScriptStartup(frame: 3, world: \"NEWWORLD/NEWWORLD\")]").unwrap();
    assert_eq!(frames(&commands), [3]);
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::ZenginScriptStartup { world, clean_world: false, .. }
            if world == "NEWWORLD/NEWWORLD"
    ));

    let commands =
        parse("[ZenginScriptStartup(frame: 3, clean_world: true, world: \"NEWWORLD/NEWWORLD\")]")
            .unwrap();
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
    let commands = parse("[ZenginSetDebugNpc(frame: 3, name: \"lares\")]").unwrap();
    assert_eq!(frames(&commands), [3]);
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::ZenginSetDebugNpc { name, .. } if name == "lares"
    ));
}

#[test]
fn press_release_command_parses_and_schedules() {
    let commands = parse("[PressRelease(frame: 4, action: Gameplay(Jump))]").unwrap();
    assert_eq!(frames(&commands), [4]);
    assert!(matches!(
        commands[0].command,
        ReplayCommand::PressRelease {
            action: RecordedAction::Gameplay(GameplayAction::Jump),
            ..
        }
    ));
}

#[test]
fn spawn_npc_command_parses_and_schedules() {
    let commands = parse(
        "[ZenginSpawnNpc(name: \"lares\", translation: (1.0, 2.0, 3.0), rotation: (0.0, 90.0, 0.0))]",
    )
    .unwrap();
    assert_eq!(frames(&commands), [5]);
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::ZenginSpawnNpc { name, translation, .. }
            if name == "lares" && *translation == Vec3::new(1.0, 2.0, 3.0)
    ));
}

#[test]
fn spawn_mob_command_parses_and_schedules() {
    let commands = parse(
        "[ZenginSpawnMob(visual: \"CHAIR_NW_NORMAL_01\", translation: (0.0, 0.0, -2.0), rotation: (0.0, 0.0, 0.0))]",
    )
    .unwrap();
    assert_eq!(frames(&commands), [5]);
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::ZenginSpawnMob { visual, state_num: None, focus_name: None, translation, .. }
            if visual == "CHAIR_NW_NORMAL_01"
                && *translation == Vec3::new(0.0, 0.0, -2.0)
    ));

    let commands = parse(
        "[ZenginSpawnMob(visual: \"chair\", focus_name: \"Chair\", state_num: 2, translation: (1.0, 2.0, 3.0), rotation: (0.0, 90.0, 0.0))]",
    )
    .unwrap();
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::ZenginSpawnMob { focus_name: Some(name), state_num: Some(2), .. }
            if name == "Chair"
    ));
}

#[test]
fn absolute_frame_serializes_without_option_wrapper() {
    let command = ReplayCommand::ExitGame {
        frame: Some(3),
        frame_dt: None,
    };
    let text = RonOptions::default()
        .with_default_extension(Extensions::IMPLICIT_SOME)
        .to_string_pretty(&vec![command], ron::ser::PrettyConfig::default())
        .unwrap();
    assert!(text.contains("frame: 3"));
    assert!(!text.contains("Some("));
    assert!(!text.contains("frame_dt"));
}

#[test]
fn identical_screenshots_match() {
    let image = test_image([20, 40, 60]);
    let report = compare_images(&image, &image.clone());
    assert!(report.changed_fraction < f32::EPSILON);
    assert!(report.mean_diff < f32::EPSILON);
}

#[test]
fn large_change_is_detected() {
    let black = test_image([0, 0, 0]);
    let white = test_image([255, 255, 255]);
    let report = compare_images(&black, &white);
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
    let report = compare_images(&image, &DynamicImage::ImageRgb8(noisy));
    assert!(report.changed_fraction <= COMPARE_MAX_CHANGED_FRACTION);
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
            TestClearLogBuffer(frame: 2),
            TestLogExists(frame: 4, regex: "WARN.*missing"),
            TestLogNotExists(frame: 6, regex: "ERROR.*missing"),
        ]"#,
    )
    .unwrap();
    assert_eq!(frames(&commands), [2, 4, 6]);
    assert!(matches!(
        &commands[1].command,
        ReplayCommand::TestLogExists { regex, .. } if regex == "WARN.*missing"
    ));
    assert!(matches!(
        &commands[2].command,
        ReplayCommand::TestLogNotExists { regex, .. } if regex == "ERROR.*missing"
    ));
}

#[test]
fn test_screenshot_command_parses_and_schedules() {
    let commands = parse(
        "[Screenshot(frame: 2, path: \"plain.png\"), TestScreenshot(frame: 4, path: \"tested.png\")]",
    )
    .unwrap();
    assert!(matches!(
        &commands[0].command,
        ReplayCommand::Screenshot { path, .. } if path == "plain.png"
    ));
    assert!(matches!(
        &commands[1].command,
        ReplayCommand::TestScreenshot { path, .. } if path == "tested.png"
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

fn parse(commands: &str) -> Result<Vec<ScheduledCommand>, String> {
    let commands: Vec<ReplayCommand> = RonOptions::default()
        .with_default_extension(Extensions::IMPLICIT_SOME)
        .from_str(commands)
        .expect("test ron should parse");
    schedule_commands(commands)
}

fn frames(commands: &[ScheduledCommand]) -> Vec<u64> {
    commands.iter().map(|command| command.frame).collect()
}

fn parse_scenario_file(path: &std::path::Path) {
    let contents = std::fs::read_to_string(path).unwrap();
    let commands: Vec<ReplayCommand> = RonOptions::default()
        .with_default_extension(Extensions::IMPLICIT_SOME)
        .from_str(&contents)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    schedule_commands(commands).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

fn test_image(color: [u8; 3]) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::from_pixel(128, 96, Rgb(color)))
}
