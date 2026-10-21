use std::f32::consts::PI;

use avian3d::{math::*, prelude::*};
use bevy::{ecs::query::Has, prelude::*};
use leafwing_input_manager::plugin::InputManagerSystem;
use leafwing_input_manager::prelude::ActionState;

use crate::gameplay::camera::{CameraResource, CurrentCamera};
use crate::gameplay::objects::PlayerCharacter;
use crate::input::actions::{DebugAction, DebugInputMarker, GameplayAction, GameplayInputMarker};
use crate::zengin::NPC_BODY;
use crate::zengin::STATIC_OBJECT;

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
            camera_toggle_input.after(InputManagerSystem::ManualControl),
        );

        app.add_systems(
            Update,
            (
                update_grounded,
                apply_gravity,
                movement.run_if(use_character_camera),
                apply_movement_damping,
                move_and_slide,
                apply_forces_to_dynamic_bodies,
                camera_movement_system.run_if(use_character_camera),
            )
                .chain(),
        );
    }
}

fn use_character_camera(info: Res<CameraResource>) -> bool {
    info.current_camera == CurrentCamera::ThirdPerson
}

/// A marker component indicating that an entity is using a character controller.
///
/// This also requires the entity to have a `CustomPositionIntegration` component, which is used
/// to prevent Avian from automatically applying the character's velocity to its position,
/// since the character controller will handle movement manually using move-and-slide.
#[derive(Component)]
#[require(RigidBody::Kinematic, CustomPositionIntegration)]
pub struct CharacterController;

/// Collision layer bit for colliders that exist only to be found by explicit
/// spatial queries (like npc selection), they must never block or carry the
/// character controller: [`move_and_slide`] and [`update_grounded`] mask this
/// layer out of their query filters.
/// See also [`crate::zengin::zengin_resources::SELECTABLE`].
pub const QUERY_ONLY_LAYER: u32 = 1 << 3;

/// Marks the player physics body entity driven by [`CharacterController`].
#[derive(Default, Component)]
pub struct PlayerPhysicsBodyMarker;

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
    /// The shape cast collider used for ground detection.
    pub cast_shape: Option<Collider>,
}

impl Default for GroundDetection {
    fn default() -> Self {
        Self {
            max_angle: PI / 6.0,
            max_distance: 0.2,
            cast_shape: None,
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

/// Toggles between the free camera and the character camera.
fn camera_toggle_input(
    mut info: ResMut<CameraResource>,
    action_state: Single<&ActionState<DebugAction>, With<DebugInputMarker>>,
    camera_query: Query<&GlobalTransform, (With<Camera3d>, Without<PlayerPhysicsBodyMarker>)>,
    mut player_query: Query<&mut Transform, With<PlayerPhysicsBodyMarker>>,
    mut player_visual: Query<
        &mut Transform,
        (
            With<PlayerCharacter>,
            Without<PlayerPhysicsBodyMarker>,
            Without<Camera3d>,
        ),
    >,
) {
    if action_state.just_pressed(&DebugAction::ToggleCharacterCamera) {
        let next_camera = match info.current_camera {
            CurrentCamera::Free => CurrentCamera::ThirdPerson,
            CurrentCamera::ThirdPerson => CurrentCamera::Free,
            CurrentCamera::Dialog => CurrentCamera::Free,
        };
        if next_camera == CurrentCamera::ThirdPerson {
            // Player starts where the free camera was looking
            if let (Ok(camera_transform), Ok(mut player_transform)) =
                (camera_query.single(), player_query.single_mut())
            {
                player_transform.translation = camera_transform.translation();
                // The camera looks along its forward; make the character face
                // that direction (the model carries the facing, not the body).
                if let Ok(mut visual_transform) = player_visual.single_mut() {
                    let forward = camera_transform.forward();
                    let dir = Vec3::new(forward.x, 0.0, forward.z);
                    if dir.length_squared() > f32::EPSILON {
                        let dir = dir.normalize();
                        visual_transform.rotation = Quat::from_rotation_y(dir.x.atan2(dir.z));
                    }
                }
            }
        }
        info.current_camera = next_camera;
    }
}

/// Updates the [`Grounded`] status for character controllers.
fn update_grounded(
    mut commands: Commands,
    mut query: Query<(Entity, &GroundDetection, &GlobalTransform)>,
    spatial_query: SpatialQuery,
) {
    for (entity, ground_detection, global_transform) in &mut query {
        let Some(collider) = &ground_detection.cast_shape else {
            continue;
        };

        let translation = global_transform.translation();
        let rotation = global_transform.rotation();

        // Cast the shape downward to check for ground
        let hit = spatial_query.cast_shape(
            collider,
            translation,
            rotation,
            global_transform.down(),
            &ShapeCastConfig::from_max_distance(ground_detection.max_distance),
            &SpatialQueryFilter::from_excluded_entities([entity])
                .with_mask(!(QUERY_ONLY_LAYER | NPC_BODY)),
        );

        // The character is grounded if we hit a surface that isn't too steep
        let is_grounded = hit.is_some_and(|hit| {
            let up = global_transform.up();
            (rotation * hit.normal1).angle_between(*up) <= ground_detection.max_angle
        });

        // Update grounded state
        if is_grounded {
            commands.entity(entity).insert(Grounded);
        } else {
            commands.entity(entity).remove::<Grounded>();
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
fn movement(
    time: Res<Time>,
    mut commands: Commands,
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    mut controllers: Query<(
        Entity,
        &CharacterMovementSettings,
        &mut LinearVelocity,
        &Transform,
        Has<Grounded>,
    )>,
) {
    let delta_secs = time.delta_secs();

    // Keyboard D-pads and the gamepad stick are summed, so clamp to unit length.
    let direction = action_state
        .axis_pair(&GameplayAction::Move)
        .clamp_length_max(1.0);
    let is_moving = direction.length_squared() >= 0.1;
    let jump_pressed = action_state.just_pressed(&GameplayAction::Jump);
    let accelerating = action_state.pressed(&GameplayAction::Accelerate);

    for (entity, movement, mut linear_velocity, tr, is_grounded) in &mut controllers {
        // Accelerate scales the animation's root motion delta movement (read
        // by the animation system via [`AcceleratingRootMotion`]).
        if accelerating {
            commands.entity(entity).insert(AcceleratingRootMotion);
        } else {
            commands.entity(entity).remove::<AcceleratingRootMotion>();
        }

        if is_moving {
            let rot = tr.rotation.to_euler(EulerRot::YXZ);
            let dir = Vec3 {
                x: -direction.x,
                y: 0.0,
                z: direction.y,
            }
            .rotate_y(rot.0);
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
fn apply_gravity(
    time: Res<Time>,
    mut controllers: Query<(&CharacterMovementSettings, &mut LinearVelocity)>,
) {
    let delta_secs = time.delta_secs();

    for (movement, mut linear_velocity) in &mut controllers {
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
fn apply_movement_damping(
    mut query: Query<(&CharacterMovementSettings, &mut LinearVelocity)>,
    time: Res<Time>,
) {
    let delta_secs = time.delta_secs();

    for (movement, mut linear_velocity) in &mut query {
        // Approximate exponential decay. We could use `LinearDamping` for this,
        // but we don't want to dampen movement along the Y axis.
        linear_velocity.x *= 1.0 / (1.0 + delta_secs * movement.damping);
        linear_velocity.z *= 1.0 / (1.0 + delta_secs * movement.damping);
    }
}

/// Performs move-and-slide for character controllers, moving them according to their velocity
/// and sliding along any contact surfaces. Also updates the [`Grounded`] state.
///
/// For simplicity, we assume that the character is not a child entity,
/// and its collider is on the same entity as the `CharacterController`.
#[allow(clippy::type_complexity)]
pub fn move_and_slide(
    mut query: Query<
        (
            Entity,
            Option<&GroundDetection>,
            Option<&mut CharacterCollisions>,
            &mut Transform,
            &mut LinearVelocity,
            &Collider,
        ),
        With<CharacterController>,
    >,
    move_and_slide: MoveAndSlide,
    time: Res<Time>,
) {
    for (entity, ground_detection, mut collisions, mut transform, mut lin_vel, collider) in
        &mut query
    {
        let mut hit_ground_or_ceiling = false;

        if let Some(collisions) = &mut collisions {
            // Clear previous collisions
            collisions.0.clear();
        }

        let up = *transform.up();

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
            &MoveAndSlideConfig::default(),
            // Selection and npc hit colliders are not physical surfaces, the
            // player would collide with its own hit volume and the invisible
            // capsules of nearby npcs.
            &SpatialQueryFilter::from_excluded_entities([entity])
                .with_mask(!(QUERY_ONLY_LAYER | NPC_BODY)),
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
fn split_into_components(v: Vec3, up: Vec3) -> [Vec3; 2] {
    let vertical_component = up * v.dot(up);
    let horizontal_component = v - vertical_component;
    [horizontal_component, vertical_component]
}

/// Applies forces to dynamic rigid bodies hit by character controllers based on their collisions.
fn apply_forces_to_dynamic_bodies(
    characters: Query<(&ComputedMass, &CharacterCollisions)>,
    colliders: Query<&ColliderOf>,
    mut rigid_bodies: Query<(&RigidBody, Forces)>,
) {
    for (mass, collisions) in &characters {
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

            let touch_dir = -collision.normal;
            let relative_velocity = collision.character_velocity - forces.linear_velocity();
            let touch_velocity = touch_dir.dot(relative_velocity) * touch_dir;
            let impulse = touch_velocity * mass;

            forces.apply_linear_impulse_at_point(impulse, collision.point);
        }
    }
}

/// Maximum distance of the third person camera from the player.
const MAX_CAM_DISTANCE: f32 = 6.0;
/// Safety margin subtracted from the camera collision distance so the near
/// plane does not touch the hit surface.
const CAM_COLLISION_MARGIN: f32 = 0.3;
/// Smallest allowed camera distance, keeps the camera from reaching the pivot
/// when the player is very close to geometry.
const MIN_CAM_DISTANCE: f32 = 0.5;
/// Height of the camera pivot above the player origin.
const CAM_PIVOT_HEIGHT: f32 = 1.3;

pub fn camera_movement_system(
    player_query: Single<(&Transform, Entity), (With<PlayerPhysicsBodyMarker>, Without<Camera3d>)>,
    mut camera_query: Query<&mut Transform, (With<Camera3d>, Without<PlayerPhysicsBodyMarker>)>,
    spatial_query: SpatialQuery,
) {
    let (player_transform, player_entity) = *player_query;
    let mut camera_transform = camera_query.single_mut().unwrap();

    let offset_direction = camera_transform.rotation * Vec3::Z;
    let pivot = player_transform.translation + Vec3::Y * CAM_PIVOT_HEIGHT;
    let Ok(ray_direction) = Dir3::new(offset_direction) else {
        warn!(
            "Third person camera offset direction is not a valid unit direction: {:?}",
            offset_direction
        );
        return;
    };

    // Cast a ray from the pivot towards the camera position and clamp the
    // distance so the camera never goes behind static world geometry.
    let distance = match spatial_query.cast_ray(
        pivot,
        ray_direction,
        MAX_CAM_DISTANCE,
        false,
        &SpatialQueryFilter::from_excluded_entities([player_entity]).with_mask(STATIC_OBJECT),
    ) {
        Some(hit) => (hit.distance - CAM_COLLISION_MARGIN).max(MIN_CAM_DISTANCE),
        None => MAX_CAM_DISTANCE,
    };

    camera_transform.translation = pivot + offset_direction * distance;

    // The player rotation is owned by the human animation sequencer (model
    // facing); the body keeps its rotation so the model's global facing and
    // root motion direction stay camera independent.
}
