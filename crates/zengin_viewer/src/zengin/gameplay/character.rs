use std::f32::consts::PI;
use std::time::Instant;

use avian3d::{math::*, prelude::*};
use bevy::{ecs::query::Has, prelude::*};
use leafwing_input_manager::prelude::ActionState;

use crate::input::{GameplayAction, GameplayInputMarker, world_input_enabled};
use crate::zengin::AppState;
use crate::zengin::gameplay::camera::CameraResource;
use crate::zengin::gameplay::character_camera::{
    camera_movement_system, camera_toggle_input, use_character_camera,
};
use crate::zengin::{NPC_BODY, NpcMobUse, SELECTABLE};
use dev_util::perf_debug;

/// A plugin that implements a basic platformer kinematic character controller using move-and-slide,
/// with support for ground detection and configurable movement settings.
pub struct CharacterControllerPlugin;

impl Plugin for CharacterControllerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CameraResource::default());

        // Toggles in `PreUpdate` so the `Update` systems see the new value this frame.
        // Runs after leafwing's action update and after replayed presses so this
        // frame's actions are read either way.
        app.add_systems(
            PreUpdate,
            camera_toggle_input
                .after(leafwing_input_manager::plugin::InputManagerSystem::ManualControl)
                .run_if(in_state(AppState::InGame)),
        );

        app.add_systems(
            Update,
            (
                update_npc_physics_active,
                update_grounded,
                apply_gravity,
                movement
                    .run_if(use_character_camera)
                    .run_if(world_input_enabled),
                apply_movement_damping,
                move_and_slide,
                apply_forces_to_dynamic_bodies,
                camera_movement_system.run_if(use_character_camera),
            )
                .chain()
                .run_if(in_state(AppState::InGame)),
        );
    }
}

/// A marker component indicating that an entity is using a character controller.
///
/// This also requires the entity to have a `CustomPositionIntegration` component, which is used
/// to prevent Avian from automatically applying the character's velocity to its position,
/// since the character controller will handle movement manually using move-and-slide.
#[derive(Component)]
#[require(RigidBody::Kinematic, CustomPositionIntegration)]
pub struct CharacterController;

/// A kinematic npc body swept by [`move_and_slide`]. Unlike
/// [`CharacterController`] it is moved by the animation root motion instead of
/// input; gravity, ground detection and damping are shared with the player
/// systems.
///
/// The collider must be on the same entity, like [`CharacterController`].
#[derive(Component)]
#[require(RigidBody::Kinematic, CustomPositionIntegration)]
pub(crate) struct NpcPhysicsMarker;

/// Present while an npc body is close enough to the camera to be simulated.
/// Far npcs keep their body components but are skipped by the sweep and gravity
/// systems, so the per-frame cost stays bounded.
#[derive(Component)]
pub(crate) struct NpcPhysicsActiveMarker;

/// Camera distance at which npc physics starts being simulated.
const NPC_PHYSICS_ACTIVE_DISTANCE: f32 = 40.0;
const NPC_PHYSICS_ACTIVE_DISTANCE_SQUARED: f32 =
    NPC_PHYSICS_ACTIVE_DISTANCE * NPC_PHYSICS_ACTIVE_DISTANCE;
/// Larger release distance, so npcs at the edge do not toggle every frame.
const NPC_PHYSICS_RELEASE_DISTANCE: f32 = 45.0;
const NPC_PHYSICS_RELEASE_DISTANCE_SQUARED: f32 =
    NPC_PHYSICS_RELEASE_DISTANCE * NPC_PHYSICS_RELEASE_DISTANCE;

/// An npc body with a velocity magnitude (in m/s, squared) below this is
/// considered idle and skips the sweep.
const NPC_IDLE_VELOCITY_SQUARED: f32 = 0.01;

/// Ground steeper than this counts as angled, and the body is swept every frame
/// so it slides along the slope instead of floating or sinking.
const FLAT_GROUND_MAX_ANGLE: f32 = 10.0 * PI / 180.0;

/// Extra distance checked ahead along the frame's movement before a sweep is
/// skipped, so an obstacle slightly further than this frame's travel still gets
/// a real sweep.
const FAST_PATH_CLEARANCE_MARGIN: f32 = 0.05;

/// How often a body gets an authoritative full sweep regardless of its state.
const GROUND_PROBE_PERIOD_SECS: f32 = 2.0;

/// Distance from the player entity origin (model root/feet) up to the capsule
/// collider center.
pub const PLAYER_MODEL_ROOT_TO_CAPSULE_CENTER: f32 = 1.0;

/// Radius of the npc physics capsule.
pub(crate) const NPC_PHYSICS_RADIUS: f32 = 0.3;

/// Cylinder length of the npc physics capsule, the caps add twice the radius,
/// so the total height is 1.8 m like a human npc.
pub(crate) const NPC_PHYSICS_LENGTH: f32 = 1.2;

/// Vertical offset of the npc physics capsule center above the npc model root.
/// The npc entity origin sits below the model root, see `NpcBodyRootOffset`.
pub(crate) const NPC_PHYSICS_CENTER_ABOVE_ROOT: f32 = 0.9;

/// Component for configuring movement settings for a character controller.
#[derive(Component)]
pub struct CharacterMovementSettings {
    /// The acceleration used for character movement.
    pub acceleration: f32,
    /// The damping coefficient used for slowing down movement.
    pub damping: f32,
    /// The strength of a jump.
    pub jump_impulse: f32,
    /// The gravitational acceleration used for the character.
    pub gravity: Vec3,
    /// The maximum speed that gravity can accelerate the character to.
    /// This prevents the character from accelerating indefinitely while falling.
    pub terminal_velocity: f32,
}

impl Default for CharacterMovementSettings {
    fn default() -> Self {
        Self {
            acceleration: 35.0,
            damping: 10.0,
            jump_impulse: 7.0,
            gravity: Vec3::new(0.0, -9.81 * 2.0, 0.0),
            terminal_velocity: 100.0,
        }
    }
}

/// Component for configuring ground detection for a character controller.
#[derive(Component)]
pub struct GroundDetection {
    /// The maximum angle (in radians) where a surface is considered ground/ceiling
    /// relative to the up-direction. Outside of this angle, surfaces are considered walls.
    ///
    /// **Default**: 30 degrees (π / 6 radians)
    pub max_angle: f32,
    /// The maximum distance for ground detection.
    pub max_distance: f32,
}

impl Default for GroundDetection {
    fn default() -> Self {
        Self {
            max_angle: PI / 6.0,
            max_distance: 0.2,
        }
    }
}

/// A marker component indicating that an entity is on a surface that is considered
/// ground, meaning the steepness is less than [`GroundDetection::max_angle`].
///
/// Characters that are grounded can jump, and do not slide down slopes.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Grounded;

/// Gap between the physical collider bottom and the ground surface, in meters.
///
/// Written by [`update_grounded`] and used by the sweep-free fast path in
/// [`move_and_slide`] to follow the terrain height.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub(crate) struct GroundContact {
    distance: f32,
}

/// The ground below the body is steeper than [`FLAT_GROUND_MAX_ANGLE`], so the
/// body needs a full sweep every frame to slide along the slope.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub(crate) struct AngledGround;

/// Counts up to [`GROUND_PROBE_PERIOD_SECS`] so the body gets an authoritative
/// full sweep even when it keeps using the sweep-free fast path.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub(crate) struct GroundProbeTimer {
    elapsed_secs: f32,
}

/// The body translation at the end of the previous frame, used to include
/// direct transform movement (animation root motion) in the fast-path clearance
/// check.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub(crate) struct PreviousTranslation(Vec3);

/// A component containing information about the current collisions for a character controller.
///
/// This is used to apply forces to dynamic rigid bodies hit by the character.
#[derive(Component, Default, Deref)]
pub struct CharacterCollisions(Vec<CharacterCollision>);

/// Information about a collision between a character controller and another collider.
pub struct CharacterCollision {
    /// The collider that was hit by the character.
    pub collider: Entity,
    /// The point of contact in world space.
    pub point: Vec3,
    /// The normal of the contact surface, pointing away from the character.
    pub normal: Dir3,
    /// The velocity of the character at the point of contact.
    pub character_velocity: Vec3,
}

/// Updates the [`NpcPhysicsActiveMarker`] of the npc bodies near the camera.
fn update_npc_physics_active(
    mut commands: Commands,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    npcs: Query<(Entity, &GlobalTransform, Has<NpcPhysicsActiveMarker>), With<NpcPhysicsMarker>>,
) {
    let perf = perf_debug!();
    let Some(camera) = cameras.iter().next() else {
        return;
    };
    let camera_position = camera.translation();
    perf.record_count("npcs", npcs.iter().len());
    for (entity, transform, active) in &npcs {
        let limit = if active {
            NPC_PHYSICS_RELEASE_DISTANCE_SQUARED
        } else {
            NPC_PHYSICS_ACTIVE_DISTANCE_SQUARED
        };
        let in_range = camera_position.distance_squared(transform.translation()) <= limit;
        match (in_range, active) {
            (true, false) => {
                perf.increment_tag("activated", 1);
                commands.entity(entity).insert(NpcPhysicsActiveMarker);
            }
            (false, true) => {
                perf.increment_tag("released", 1);
                commands
                    .entity(entity)
                    // A body that leaves the range must not resume with stale motion.
                    .insert(LinearVelocity::default())
                    .remove::<NpcPhysicsActiveMarker>()
                    .remove::<Grounded>()
                    .remove::<AngledGround>()
                    .remove::<GroundContact>()
                    .remove::<GroundProbeTimer>();
            }
            _ => {}
        }
    }
}

/// Updates the [`Grounded`] status for character controllers and camera-near
/// npc bodies, and classifies the ground for the sweep-free fast path.
#[allow(clippy::type_complexity)]
fn update_grounded(
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            &GroundDetection,
            &GlobalTransform,
            Has<Grounded>,
            Has<AngledGround>,
            Option<&mut GroundContact>,
            Option<&Collider>,
        ),
        (
            Without<NpcMobUse>,
            Or<(Without<NpcPhysicsMarker>, With<NpcPhysicsActiveMarker>)>,
        ),
    >,
    spatial_query: SpatialQuery,
) {
    let perf = perf_debug!();
    for (entity, ground_detection, global_transform, grounded, angled, contact, body_collider) in
        &mut query
    {
        perf.increment_tag("checked", 1);

        let translation = global_transform.translation();
        let rotation = global_transform.rotation();

        // A single downward ray from the physical capsule bottom is enough to
        // tell whether the body rests on flat ground. The periodic full sweep
        // catches ledges and edges the center ray misses.
        let body_bottom = body_collider.map_or(translation.y, |body_collider| {
            body_collider.aabb(translation, rotation, 0.0).min.y
        });
        let ray_origin = translation + *global_transform.up() * (body_bottom - translation.y);
        let hit = spatial_query.cast_ray(
            ray_origin,
            global_transform.down(),
            ground_detection.max_distance,
            true,
            &SpatialQueryFilter::from_excluded_entities([entity])
                .with_mask(!(SELECTABLE | NPC_BODY)),
        );

        // The character is grounded if we hit a surface that isn't too steep
        let up = global_transform.up();
        let Some(hit) =
            hit.filter(|hit| hit.normal.angle_between(*up) <= ground_detection.max_angle)
        else {
            if grounded {
                commands.entity(entity).remove::<Grounded>();
            }
            if angled {
                commands.entity(entity).remove::<AngledGround>();
            }
            if contact.is_some() {
                commands.entity(entity).remove::<GroundContact>();
            }
            continue;
        };

        perf.increment_tag("grounded", 1);
        if !grounded {
            commands.entity(entity).insert(Grounded);
        }

        // A steep slope has to be swept every frame, otherwise the flat fast
        // path would walk into the hill instead of sliding along it.
        let is_angled = hit.normal.angle_between(*up) > FLAT_GROUND_MAX_ANGLE;
        if is_angled {
            perf.increment_tag("angled", 1);
            if !angled {
                commands.entity(entity).insert(AngledGround);
            }
        } else if angled {
            commands.entity(entity).remove::<AngledGround>();
        }

        match contact {
            Some(mut contact) => contact.distance = hit.distance,
            None => {
                commands.entity(entity).insert(GroundContact {
                    distance: hit.distance,
                });
            }
        }
    }
}

/// Acceleration multiplier while the `Accelerate` action is held.
pub(crate) const ACCELERATION_MULTIPLIER: f32 = 5.0;

/// Marks a root motion driven physics body whose animation movement is
/// currently accelerated (the player holds the `Accelerate` action). Read by
/// the animation system to scale the animation's root motion.
#[derive(Component, Debug)]
pub(crate) struct AcceleratingRootMotion;

/// Responds to the movement [`GameplayAction`]s and moves character controllers accordingly.
#[allow(clippy::type_complexity)]
fn movement(
    time: Res<Time>,
    mut commands: Commands,
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    mut controllers: Query<
        (
            Entity,
            &CharacterMovementSettings,
            &mut LinearVelocity,
            Has<Grounded>,
        ),
        (With<CharacterController>, Without<NpcMobUse>),
    >,
) {
    let perf = perf_debug!();
    let delta_secs = time.delta_secs();

    // Keyboard D-pads and the gamepad stick are summed, so clamp to unit length.
    let direction = action_state
        .axis_pair(&GameplayAction::Move)
        .clamp_length_max(1.0);
    let is_moving = direction.length_squared() >= 0.1;
    let jump_pressed = action_state.just_pressed(&GameplayAction::Jump);
    let accelerating = action_state.pressed(&GameplayAction::Accelerate);

    for (entity, movement, mut linear_velocity, is_grounded) in &mut controllers {
        perf.increment_tag("controllers", 1);
        // Accelerate scales the animation's root motion delta movement (read
        // by the animation system via [`AcceleratingRootMotion`]).
        if accelerating {
            commands.entity(entity).insert(AcceleratingRootMotion);
        } else {
            commands.entity(entity).remove::<AcceleratingRootMotion>();
        }

        if is_moving {
            // Input stays in world axes: the entity yaw is the model facing,
            // so rotating the input by it would feed facing back into the
            // movement direction.
            let dir = Vec3 {
                x: -direction.x,
                y: 0.0,
                z: direction.y,
            };
            let acceleration = if accelerating {
                movement.acceleration * ACCELERATION_MULTIPLIER
            } else {
                movement.acceleration
            };
            linear_velocity.x += dir.x * acceleration * delta_secs;
            linear_velocity.z += dir.z * acceleration * delta_secs;
        }

        if jump_pressed && is_grounded {
            linear_velocity.y = movement.jump_impulse;
        }
    }
}

/// Applies gravity to character controllers.
#[allow(clippy::type_complexity)]
fn apply_gravity(
    time: Res<Time>,
    mut controllers: Query<
        (
            &CharacterMovementSettings,
            &mut LinearVelocity,
            Has<CharacterController>,
            Has<Grounded>,
        ),
        (
            Or<(With<CharacterController>, With<NpcPhysicsActiveMarker>)>,
            Without<NpcMobUse>,
        ),
    >,
) {
    let perf = perf_debug!();
    let delta_secs = time.delta_secs();

    for (movement, mut linear_velocity, is_controller, grounded) in &mut controllers {
        perf.increment_tag("bodies", 1);
        // An npc resting on the ground must not accumulate downward velocity,
        // otherwise it would need a sweep every frame just to cancel it.
        if !is_controller && grounded {
            perf.increment_tag("grounded_skip", 1);
            continue;
        }
        let gravity_direction = movement.gravity.normalize_or_zero();

        let velocity_along_gravity = linear_velocity.dot(gravity_direction);
        if velocity_along_gravity > movement.terminal_velocity {
            // Don't apply more gravity if we're already at terminal velocity.
            continue;
        }

        // Calculate the new velocity after applying gravity.
        let new_velocity = linear_velocity.0 + movement.gravity * delta_secs;

        // Don't exceed terminal velocity.
        let new_velocity_along_gravity = new_velocity.dot(gravity_direction);
        if new_velocity_along_gravity < movement.terminal_velocity {
            linear_velocity.0 = new_velocity;
        } else {
            linear_velocity.0 = gravity_direction * movement.terminal_velocity;
        }
    }
}

/// Slows down movement in the XZ plane.
#[allow(clippy::type_complexity)]
fn apply_movement_damping(
    mut query: Query<
        (&CharacterMovementSettings, &mut LinearVelocity),
        (With<CharacterController>, Without<NpcMobUse>),
    >,
    time: Res<Time>,
) {
    let perf = perf_debug!();
    let delta_secs = time.delta_secs();

    for (movement, mut linear_velocity) in &mut query {
        perf.increment_tag("controllers", 1);
        // Approximate exponential decay. We could use `LinearDamping` for this,
        // but we don't want to dampen movement along the Y axis.
        linear_velocity.x *= 1.0 / (1.0 + delta_secs * movement.damping);
        linear_velocity.z *= 1.0 / (1.0 + delta_secs * movement.damping);
    }
}

/// Performs move-and-slide for character controllers, moving them according to their velocity
/// and sliding along any contact surfaces.
///
/// Bodies resting on flat ground skip the sweep as long as a single clearance
/// cast along their horizontal movement stays clear, and follow the ground
/// height from [`GroundContact`]. Every [`GROUND_PROBE_PERIOD_SECS`] the probe
/// forces one full sweep so the body cannot drift out of sync.
///
/// For simplicity, we assume that the character is not a child entity,
/// and its collider is on the same entity as the `CharacterController`.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn move_and_slide(
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            Option<&GroundDetection>,
            Option<&mut CharacterCollisions>,
            &mut Transform,
            &mut LinearVelocity,
            &Collider,
            Has<CharacterController>,
            Has<Grounded>,
            Has<AngledGround>,
            Option<&mut GroundProbeTimer>,
            Option<&mut PreviousTranslation>,
            Option<&GroundContact>,
        ),
        (
            Or<(With<CharacterController>, With<NpcPhysicsActiveMarker>)>,
            Without<NpcMobUse>,
        ),
    >,
    move_and_slide: MoveAndSlide,
    time: Res<Time>,
) {
    let perf = perf_debug!();
    let move_and_slide_config = MoveAndSlideConfig::default();
    let mut probe_effort_us: u64 = 0;
    for (
        entity,
        ground_detection,
        mut collisions,
        mut transform,
        mut lin_vel,
        collider,
        is_controller,
        grounded,
        angled,
        mut probe_timer,
        mut previous_translation,
        ground_contact,
    ) in &mut query
    {
        perf.increment_tag("bodies", 1);
        let is_npc = !is_controller;
        // A body with a negligible velocity does not need a sweep. The vertical
        // component matters too: an idle npc dropped in the air has no horizontal
        // velocity but still has to fall.
        if is_npc && lin_vel.0.length_squared() < NPC_IDLE_VELOCITY_SQUARED {
            perf.increment_tag("idle_skip", 1);
            continue;
        }

        // A body without a timer or recorded position has never been swept, so
        // it gets a full sweep first.
        let mut force_sweep = probe_timer.is_none() || previous_translation.is_none();
        match &mut probe_timer {
            Some(timer) => {
                timer.elapsed_secs += time.delta_secs();
                if timer.elapsed_secs >= GROUND_PROBE_PERIOD_SECS {
                    timer.elapsed_secs = 0.0;
                    force_sweep = true;
                }
            }
            None => {
                commands.entity(entity).insert(GroundProbeTimer::default());
            }
        }
        if previous_translation.is_none() {
            commands
                .entity(entity)
                .insert(PreviousTranslation(transform.translation));
        }
        if force_sweep {
            perf.increment_tag("probe", 1);
        }

        if let Some(collisions) = &mut collisions {
            // Clear previous collisions
            collisions.0.clear();
        }

        let up = *transform.up();

        // Sweep-free fast path: the body is on flat ground and nothing blocks
        // the horizontal movement. Moving upward (a jump) always needs the full
        // sweep so the jump is not cancelled here.
        if !force_sweep
            && grounded
            && !angled
            && let Some(ground_contact) = ground_contact
            && lin_vel.dot(up) <= 0.0
        {
            // Animation root motion moves the player transform directly, so the
            // movement since the last frame is part of the path to check.
            let origin = previous_translation
                .as_ref()
                .map_or(transform.translation, |previous| previous.0);
            let moved = transform.translation - origin;
            let actual_horizontal = moved - up * moved.dot(up);
            let horizontal = lin_vel.0 - up * lin_vel.dot(up);
            let travel = actual_horizontal + horizontal * time.delta_secs();
            if horizontal_path_is_clear(
                &move_and_slide.spatial_query,
                collider,
                &transform,
                origin,
                travel,
                entity,
            ) {
                perf.increment_tag("fast_path", 1);
                transform.translation += horizontal * time.delta_secs();
                follow_ground_height(
                    &mut transform,
                    ground_contact,
                    move_and_slide.length_unit.0 * move_and_slide_config.skin_width,
                );
                // Cancel the gravity accumulated while standing on the ground;
                // the full sweep is what normally does this.
                let vertical = lin_vel.dot(up);
                if vertical < 0.0 {
                    lin_vel.0 -= up * vertical;
                }
                if let Some(previous) = previous_translation.as_mut() {
                    previous.0 = transform.translation;
                }
                continue;
            }
            perf.increment_tag("clearance_blocked", 1);
        }

        let sweep_start = Instant::now();
        perf.increment_tag("swept", 1);

        let mut hit_ground_or_ceiling = false;

        // Perform move-and-slide
        let MoveAndSlideOutput {
            position: new_position,
            projected_velocity,
        } = move_and_slide.move_and_slide(
            collider,
            transform.translation,
            transform.rotation,
            lin_vel.0,
            time.delta(),
            &move_and_slide_config,
            // Selection and npc hit colliders are not physical surfaces, the
            // player would collide with its own hit volume and the invisible
            // capsules of nearby npcs.
            &SpatialQueryFilter::from_excluded_entities([entity])
                .with_mask(!(SELECTABLE | NPC_BODY)),
            |hit| {
                // This callback is called for each surface we collide with during move-and-slide.
                // In this example, we use it to customize collision behavior for ground surfaces,
                // preventing sliding down slopes when we are grounded, and preventing climbing up steep slopes.

                let Some(ground_detection) = ground_detection else {
                    // Early out if we don't have ground detection.
                    return MoveAndSlideHitResponse::Accept;
                };

                // Determine if the surface is ground based on the angle between the up-vector and the hit normal.
                let angle = up.angle_between(**hit.normal);
                let is_ground = angle <= ground_detection.max_angle;
                let is_ceiling = is_ground && up.dot(**hit.normal) < 0.0;

                // Decompose the original input velocity into components relative to the hit normal and the up direction,
                // to determine how much of the velocity is contributing to climbing, slipping, and unconstrained movement.
                let [horizontal_component, vertical_component] =
                    split_into_components(lin_vel.0, up);

                // Decompose the horizontal component and the current sliding velocity to determine
                // whether the character is trying to climb or slip, and whether it is actually climbing or slipping.
                let horizontal_velocity_decomposition =
                    decompose_hit_velocity(horizontal_component, *hit.normal, up);
                let decomposition = decompose_hit_velocity(*hit.velocity, *hit.normal, up);

                // An object is trying to slip if the tangential movement induced by its vertical movement
                // points downward (with a small threshold).
                let slipping_intent =
                    up.dot(horizontal_velocity_decomposition.vertical_tangent) < -0.001;

                // An object is slipping if its vertical movement points downward (with a small threshold).
                let slipping = up.dot(decomposition.vertical_tangent) < -0.001;

                // An object is trying to climb if its vertical input motion points upward.
                let climbing_intent = up.dot(vertical_component) > 0.0;

                // An object is climbing if the tangential movement induced by its vertical movement points upward.
                let climbing = up.dot(decomposition.vertical_tangent) > 0.0;

                let projected_velocity = if !is_ground && climbing && !climbing_intent {
                    // Can’t climb the slope, remove the vertical tangent motion induced by the forward motion.
                    decomposition.horizontal_tangent + decomposition.normal_part
                } else if is_ground && slipping && !slipping_intent {
                    // Prevent the vertical movement from sliding down.
                    decomposition.horizontal_tangent + decomposition.normal_part
                } else {
                    // Otherwise, allow full movement (including climbing and slipping)
                    decomposition.horizontal_tangent
                        + decomposition.vertical_tangent
                        + decomposition.normal_part
                };

                // Update the current velocity used by the algorithm.
                *hit.velocity = projected_velocity;

                if is_ground || is_ceiling {
                    // We hit a ground or ceiling surface!
                    hit_ground_or_ceiling = true;
                }

                if let Some(collisions) = &mut collisions {
                    // Record the collision for use in other systems, such as applying forces to dynamic bodies.
                    collisions.0.push(CharacterCollision {
                        collider: hit.entity,
                        point: hit.point,
                        normal: *hit.normal,
                        character_velocity: *hit.velocity,
                    });
                }

                // Accept the hit and continue the move-and-slide algorithm with the modified velocity.
                MoveAndSlideHitResponse::Accept
            },
        );

        // Update position to the final position calculated by move-and-slide.
        transform.translation = new_position.f32();

        // If we hit the ground or a ceiling, update the velocity along the up-direction
        // to prevent accumulating velocity along the ground normal when hitting slopes,
        // and to prevent sticking to ceilings when jumping.
        if hit_ground_or_ceiling {
            let velocity_along_up = lin_vel.dot(up);
            let new_velocity_along_up = projected_velocity.dot(up);
            lin_vel.0 += (new_velocity_along_up - velocity_along_up) * up;
        }

        if force_sweep {
            probe_effort_us += sweep_start.elapsed().as_micros() as u64;
        }
        if let Some(previous) = previous_translation.as_mut() {
            previous.0 = transform.translation;
        }
    }

    if probe_effort_us > 0 {
        perf.record_count("probe_us", probe_effort_us as usize);
    }
}

/// The decomposition of a velocity Vec3 into parts relative to a collision normal and an up-direction.
///
/// This is used for determining how much of the velocity is contributing to climbing, slipping, and unconstrained movement.
#[derive(Debug)]
struct VelocityDecomposition {
    /// The part of the velocity that is directly against the collision normal.
    normal_part: Vec3,
    /// The part of the velocity that is tangent to the collision surface and perpendicular to the up-direction.
    horizontal_tangent: Vec3,
    /// The part of the velocity that is tangent to the collision surface and parallel to the up-direction.
    vertical_tangent: Vec3,
}

/// Decomposes a velocity Vec3 into parts relative to a collision `normal` and an `up` direction.
fn decompose_hit_velocity(velocity: Vec3, normal: Dir3, up: Vec3) -> VelocityDecomposition {
    let normal_part = normal * normal.dot(velocity);
    let tangent_part = velocity - normal_part;

    let horizontal_tangent_dir = normal.cross(up).normalize_or_zero();
    let horizontal_tangent = tangent_part.dot(horizontal_tangent_dir) * horizontal_tangent_dir;
    let vertical_tangent = tangent_part - horizontal_tangent;

    VelocityDecomposition {
        normal_part,
        horizontal_tangent,
        vertical_tangent,
    }
}

/// Splits a Vec3 into horizontal and vertical components relative to a given `up` direction.
fn split_into_components(vv: Vec3, up: Vec3) -> [Vec3; 2] {
    let vertical_component = up * vv.dot(up);
    let horizontal_component = vv - vertical_component;
    [horizontal_component, vertical_component]
}

/// Checks whether the body can take `travel` from `origin` without hitting
/// anything. Selection and npc hit colliders are masked out like in the full
/// sweep, and a small margin catches obstacles just ahead.
#[allow(clippy::too_many_arguments)]
fn horizontal_path_is_clear(
    spatial_query: &SpatialQuery,
    collider: &Collider,
    transform: &Transform,
    origin: Vec3,
    travel: Vec3,
    entity: Entity,
) -> bool {
    let Ok(direction) = Dir3::new(travel.normalize_or_zero()) else {
        // Standing still, nothing to walk into.
        return true;
    };
    let distance = travel.length() + FAST_PATH_CLEARANCE_MARGIN;
    spatial_query
        .cast_shape(
            collider,
            origin,
            transform.rotation,
            direction,
            &ShapeCastConfig::from_max_distance(distance),
            &SpatialQueryFilter::from_excluded_entities([entity])
                .with_mask(!(SELECTABLE | NPC_BODY)),
        )
        .is_none()
}

/// Keeps the physical collider at the sweep's skin gap above the ground so the
/// body follows the terrain between real sweeps.
fn follow_ground_height(transform: &mut Transform, contact: &GroundContact, skin: f32) {
    transform.translation.y += skin - contact.distance;
}

/// Applies forces to dynamic rigid bodies hit by character controllers based on their collisions.
fn apply_forces_to_dynamic_bodies(
    characters: Query<(&ComputedMass, &CharacterCollisions)>,
    colliders: Query<&ColliderOf>,
    mut rigid_bodies: Query<(&RigidBody, Forces)>,
) {
    let perf = perf_debug!();
    for (mass, collisions) in &characters {
        perf.increment_tag("characters", 1);
        let mass = mass.value();
        for collision in &collisions.0 {
            let Ok(collider_of) = colliders.get(collision.collider) else {
                continue;
            };
            let Ok((rigid_body, mut forces)) = rigid_bodies.get_mut(collider_of.body) else {
                continue;
            };
            if !rigid_body.is_dynamic() {
                continue;
            }
            perf.increment_tag("impulses", 1);

            let touch_dir = -collision.normal;
            let relative_velocity = collision.character_velocity - forces.linear_velocity();
            let touch_velocity = touch_dir.dot(relative_velocity) * touch_dir;
            let impulse = touch_velocity * mass;

            forces.apply_linear_impulse_at_point(impulse, collision.point);
        }
    }
}
