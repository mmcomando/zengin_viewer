use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;

use super::components::{
    AnimationEnds, AnimationState, AnimationTrigger, FrameState, LocomotionSequencer,
};
use super::controller::AnimationController;
use super::data::{AnimationData, AnimationSample};
use super::locomotion::{
    ATOMIC_SCALE, AnimationKind, TIME_SCALE, advance_animation, animation_kind, desired_next,
    update_animations,
};
use crate::gameplay::distance::DistanceNear;
use crate::gameplay::objects::PlayerCharacter;
use crate::zengin::human_animations::{AnimationTriggerDef, crossed_triggers};

/// Root movement dumped from the real `HUMANS-S_RUNL.MAN` (20 frames, 25 fps).
fn runl_movement() -> Vec<f32> {
    vec![
        0.0, 0.13519135, 0.27853653, 0.41702682, 0.5482346, 0.6780731, 0.8079115, 0.9398663,
        1.072817, 1.2036511, 1.3291327, 1.4368128, 1.5380194, 1.655036, 1.8134439, 1.9851096,
        2.1610076, 2.3322375, 2.4898987, 2.6249032,
    ]
}

fn runl_state(movement: Option<Vec<f32>>) -> AnimationState {
    AnimationState {
        frame_state: Arc::new(FrameState {
            start_frame: AtomicU32::new(0),
            end_frame: AtomicU32::new(1),
            mul_1000: AtomicU32::new(0),
            needs_update: AtomicBool::new(true),
        }),
        fps: 25.0,
        frames_num: 20,
        time_us: AtomicU32::new(0),
        movement,
        base_pos_node_index: 0,
    }
}

#[test]
fn plays_half_frame_at_natural_pace() {
    let state = runl_state(Some(runl_movement()));
    // Half of one frame time (1/25 s)
    let update = advance_animation(&state, 0.02).unwrap();

    assert_eq!(update.start_frame, 0);
    assert_eq!(update.end_frame, 1);
    assert!((update.mul_1000 as f32 / ATOMIC_SCALE - 0.5).abs() < 0.01);
    assert!(!update.finished);
    // Movement of half a frame: linear interpolation into the first frame step
    assert!((update.delta_movement - 0.5 * 0.13519135).abs() < 1e-4);
}

#[test]
fn advances_one_frame_per_frame_time() {
    let state = runl_state(Some(runl_movement()));
    let update = advance_animation(&state, 1.0 / 25.0).unwrap();

    // Exactly one frame time lands on frame 1
    assert_eq!(update.start_frame, 1);
    assert_eq!(update.end_frame, 2);
    assert_eq!(update.mul_1000, 0);
    assert!(!update.finished);
    assert!((update.delta_movement - 0.13519135).abs() < 1e-3);
    assert!((update.time_us as f32 / TIME_SCALE - 0.04).abs() < 1e-5);
}

#[test]
fn loop_travels_the_full_root_movement() {
    let movement = runl_movement();
    let total = movement[19] + (movement[19] - movement[18]);

    let state = runl_state(Some(movement));
    // Simulate one loop with 60 fps steps; travelled distance must match the
    // extrapolated full loop movement.
    let mut travelled = 0.0;
    for _ in 0..48 {
        let update = advance_animation(&state, 1.0 / 60.0).unwrap();
        state.time_us.store(update.time_us, Ordering::Release);
        travelled += update.delta_movement;
    }
    assert!((travelled - total).abs() < 1e-3, "travelled {travelled}");
}

#[test]
fn long_pause_wraps_multiple_loops() {
    let movement = runl_movement();
    let total = movement[19] + (movement[19] - movement[18]);

    let state = runl_state(Some(movement.clone()));
    // 1.7 s is two loops (1.6 s) plus 0.1 s into the new loop
    let update = advance_animation(&state, 1.7).unwrap();

    let expected = 2.0 * total + movement[2] + (movement[3] - movement[2]) * 0.5;
    assert!((update.delta_movement - expected).abs() < 1e-3);
    assert_eq!(update.start_frame, 2);
    assert!(update.finished);
}

#[test]
fn animation_without_root_movement_does_not_move_entity() {
    let state = runl_state(None);
    let update = advance_animation(&state, 0.02).unwrap();

    assert_eq!(update.delta_movement, 0.0);
    assert_eq!(update.start_frame, 0);
}

#[test]
fn invalid_animation_data_is_skipped() {
    let state = runl_state(None);
    let state = AnimationState {
        fps: 0.0,
        frames_num: 0,
        ..state
    };
    assert!(advance_animation(&state, 0.02).is_none());
}

/// Distinct handles backed by a real asset store
fn test_sequencer() -> (Assets<AnimationData>, LocomotionSequencer) {
    let mut assets = Assets::<AnimationData>::default();
    let idle = assets.add(AnimationData::default());
    let move_start = assets.add(AnimationData::default());
    let move_loop = assets.add(AnimationData::default());
    let move_stop = assets.add(AnimationData::default());
    (
        assets,
        LocomotionSequencer {
            idle,
            move_start: Some(move_start),
            move_loop,
            move_stop: Some(move_stop),
        },
    )
}

#[test]
fn movement_starts_with_transition_into_loop() {
    let (_assets, sequencer) = test_sequencer();
    let move_start = sequencer.move_start.clone().unwrap();

    // Standing still keeps looping the idle animation
    assert_eq!(
        animation_kind(&sequencer, &sequencer.idle),
        Some(AnimationKind::Idle)
    );
    assert_eq!(
        desired_next(&sequencer, AnimationKind::Idle, false),
        (None, true)
    );

    // Starting to move queues the start transition, replacing the idle
    // loop immediately
    assert_eq!(
        desired_next(&sequencer, AnimationKind::Idle, true),
        (Some(move_start.clone()), true)
    );

    // The start transition leads into the movement loop, playing out first
    assert_eq!(
        animation_kind(&sequencer, &move_start),
        Some(AnimationKind::MoveStart)
    );
    assert_eq!(
        desired_next(&sequencer, AnimationKind::MoveStart, true),
        (Some(sequencer.move_loop.clone()), false)
    );
}

#[test]
fn stopping_movement_transitions_back_to_idle() {
    let (_assets, sequencer) = test_sequencer();
    let move_stop = sequencer.move_stop.clone().unwrap();

    // Running keeps the movement loop
    assert_eq!(
        animation_kind(&sequencer, &sequencer.move_loop),
        Some(AnimationKind::MoveLoop)
    );
    assert_eq!(
        desired_next(&sequencer, AnimationKind::MoveLoop, true),
        (None, true)
    );

    // Stopping queues the stop transition, replacing the run loop
    // immediately
    assert_eq!(
        desired_next(&sequencer, AnimationKind::MoveLoop, false),
        (Some(move_stop.clone()), true)
    );

    // The stop transition leads back to idle, playing out first
    assert_eq!(
        animation_kind(&sequencer, &move_stop),
        Some(AnimationKind::MoveStop)
    );
    assert_eq!(
        desired_next(&sequencer, AnimationKind::MoveStop, false),
        (Some(sequencer.idle.clone()), false)
    );
    assert_eq!(
        desired_next(&sequencer, AnimationKind::MoveStop, true),
        (Some(sequencer.move_start.clone().unwrap()), false)
    );
}

#[test]
fn missing_transitions_fall_back_to_loops() {
    let (_assets, mut sequencer) = test_sequencer();
    sequencer.move_start = None;
    sequencer.move_stop = None;

    // Start of movement goes straight into the movement loop
    assert_eq!(
        desired_next(&sequencer, AnimationKind::Idle, true),
        (Some(sequencer.move_loop.clone()), true)
    );
    // Stop of movement goes straight back to idle
    assert_eq!(
        desired_next(&sequencer, AnimationKind::MoveLoop, false),
        (Some(sequencer.idle.clone()), true)
    );
}

#[test]
fn foreign_animations_are_left_alone() {
    let (mut assets, sequencer) = test_sequencer();
    let scripted = assets.add(AnimationData::default());

    // Animations the sequencer does not own (e.g. scripted actions) have
    // no kind; update_locomotion leaves their controller alone
    assert_eq!(animation_kind(&sequencer, &scripted), None);
}

/// Builds a two-frame animation so it passes the `frames_num > 1` guard
/// in `update_animations`.
fn test_animation() -> AnimationData {
    AnimationData::new(
        vec![
            AnimationSample::default(),
            AnimationSample::default(),
            AnimationSample::default(),
            AnimationSample::default(),
        ],
        vec![0],
        2,
        25.0,
    )
}

/// Builds a world with a single near skeleton for `update_animations`.
/// Time does not advance; the animation progress is pre-loaded via the
/// state's `time_us`.
fn run_update_animations(
    assets: Assets<AnimationData>,
    state: AnimationState,
    controller: AnimationController,
) -> (World, Entity) {
    let mut world = World::new();
    world.insert_resource(assets);
    // `update_animations` reads the generic `Time`, which TimePlugin
    // normally inserts alongside the specialized ones
    world.insert_resource(Time::<()>::default());
    world.init_resource::<Messages<AnimationEnds>>();
    world.init_resource::<Messages<AnimationTrigger>>();
    let model = world
        .spawn((
            DistanceNear::default(),
            state,
            controller,
            GlobalTransform::default(),
        ))
        .id();
    (world, model)
}

#[test]
fn queued_next_replaces_finished_animation() {
    let mut assets = Assets::<AnimationData>::default();
    let first = assets.add(test_animation());
    let second = assets.add(test_animation());

    // 2 s into the 0.8 s loop: the animation finished this step
    let state = runl_state(Some(runl_movement()));
    state.time_us.store(2_000_000, Ordering::Release);
    let mut controller = AnimationController::new(first.clone());
    controller.queue_next(second.clone(), false);
    let (mut world, model) = run_update_animations(assets, state, controller);
    world.run_system_once(update_animations).unwrap();

    let controller = world.get::<AnimationController>(model).unwrap();
    assert_eq!(controller.current(), &second);
    assert_eq!(controller.next(), None);
}

#[test]
fn interrupt_current_replaces_playing_animation() {
    let mut assets = Assets::<AnimationData>::default();
    let first = assets.add(test_animation());
    let second = assets.add(test_animation());

    // The animation just started, but the queued next may interrupt it
    let mut controller = AnimationController::new(first.clone());
    controller.queue_next(second.clone(), true);
    let (mut world, model) =
        run_update_animations(assets, runl_state(Some(runl_movement())), controller);
    world.run_system_once(update_animations).unwrap();

    let controller = world.get::<AnimationController>(model).unwrap();
    assert_eq!(controller.current(), &second);
    assert_eq!(controller.next(), None);
}

#[test]
fn queued_next_waits_for_current_to_end() {
    let mut assets = Assets::<AnimationData>::default();
    let first = assets.add(AnimationData::default());
    let second = assets.add(AnimationData::default());

    // The animation just started and may not be interrupted
    let mut controller = AnimationController::new(first.clone());
    controller.queue_next(second.clone(), false);
    let (mut world, model) =
        run_update_animations(assets, runl_state(Some(runl_movement())), controller);
    world.run_system_once(update_animations).unwrap();

    let controller = world.get::<AnimationController>(model).unwrap();
    assert_eq!(controller.current(), &first);
    assert_eq!(controller.next(), Some(&second));
}

#[test]
fn single_frame_animation_holds_its_frame() {
    let state = runl_state(None);
    let state = AnimationState {
        fps: 25.0,
        frames_num: 1,
        ..state
    };
    let update = advance_animation(&state, 0.04).unwrap();

    assert_eq!(update.start_frame, 0);
    assert_eq!(update.end_frame, 0);
    assert!(!update.finished);
    assert_eq!(update.delta_movement, 0.0);
}

#[test]
fn animation_ends_fires_when_entering_end_window() {
    let mut assets = Assets::<AnimationData>::default();
    let current = assets.add(test_animation());
    let state = runl_state(Some(runl_movement()));
    // 16 frames in (of 20): the next two frame step crosses into the
    // last_frame-2 window (frame 18)
    state.time_us.store(640_000, Ordering::Release);
    let (mut world, model) =
        run_update_animations(assets, state, AnimationController::new(current.clone()));
    world.entity_mut(model).insert(PlayerCharacter);
    world
        .resource_mut::<Time<()>>()
        .advance_by(Duration::from_secs_f32(0.08));

    world.run_system_once(update_animations).unwrap();

    let ends: Vec<_> = world
        .resource::<Messages<AnimationEnds>>()
        .iter_current_update_messages()
        .collect();
    assert_eq!(ends.len(), 1);
    assert_eq!(ends[0].who, model);
    assert_eq!(ends[0].what_animation, current);
}

#[test]
fn animation_messages_only_fire_for_players() {
    let mut assets = Assets::<AnimationData>::default();
    let current = assets.add(test_animation());
    let state = runl_state(Some(runl_movement()));
    state.time_us.store(640_000, Ordering::Release);
    let (mut world, _model) =
        run_update_animations(assets, state, AnimationController::new(current));

    world.run_system_once(update_animations).unwrap();

    assert_eq!(
        world
            .resource::<Messages<AnimationEnds>>()
            .iter_current_update_messages()
            .count(),
        0
    );
    assert_eq!(
        world
            .resource::<Messages<AnimationTrigger>>()
            .iter_current_update_messages()
            .count(),
        0
    );
}

#[test]
fn triggers_fire_on_frame_crossing() {
    let triggers = [AnimationTriggerDef {
        frame: 12,
        name: "BOWSHOOT".to_string(),
    }];

    let crossed: Vec<&str> = crossed_triggers(&triggers, 10.0, 13.0, 0)
        .map(|t| t.name.as_str())
        .collect();
    assert_eq!(crossed, ["BOWSHOOT"]);

    let crossed: Vec<&str> = crossed_triggers(&triggers, 13.0, 14.0, 0)
        .map(|t| t.name.as_str())
        .collect();
    assert!(crossed.is_empty());

    // A step that wrapped fires the trigger when it lies in the covered span
    // (from after `old_progress` around to `new_progress`)
    let crossed: Vec<&str> = crossed_triggers(&triggers, 1.0, 2.0, 1)
        .map(|t| t.name.as_str())
        .collect();
    assert_eq!(crossed, ["BOWSHOOT"]);

    // ... but not when the wrap skipped over it in both directions
    let crossed: Vec<&str> = crossed_triggers(&triggers, 13.0, 2.0, 1)
        .map(|t| t.name.as_str())
        .collect();
    assert!(crossed.is_empty());
}
