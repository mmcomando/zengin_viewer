//! Ranged combat: the player bow attack sequence and flying arrows.
//!
//! Pressing `Interact` with a ranged weapon drawn starts the sequence: an
//! aim-in transition, then the aim/fire loop (`S_BOWSHOOT`). The arrow spawns
//! at a fixed frame of the loop (Gothic-like), a reload transition follows and
//! another `Interact` press (or holding the key) fires again; releasing the
//! key plays the aim-out transition and hands the animation control back.

use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::gameplay::objects::{NpcModel, PlayerCharacter};
use crate::graphics::animation::{
    AnimationController, AnimationData, LocomotionSequencer, ScriptedAnimations,
};
use crate::gui::game_log::GameLog;
use crate::input::actions::{GameplayAction, GameplayInputMarker};
use crate::zengin::common::{choose_animation_scheme, get_animation_path};
use crate::zengin::interaction::NpcDied;
use crate::zengin::npc_routines::{AnimationFiles, NpcOverlay, NpcWeaponMode};
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::WeaponMode;
use crate::zengin::script::symbol_id::InstanceId;
use crate::zengin::selection::{NpcHitCollider, Selection};
use crate::zengin::world::item_model_path;
use crate::zengin::zengin_resources::{ARROW, MaterialHandles, NPC_BODY, ZenGinModelComponent};
use crate::zengin::{GameNpc, ZenginVfs};
use avian3d::prelude::*;

pub struct RangedCombatPlugin;

impl Plugin for RangedCombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ArrowShot>();
        app.add_systems(Update, add_bow_attack);
        app.add_systems(Update, bow_attack_state_machine.in_set(ScriptedAnimations));
        app.add_systems(Update, (spawn_arrows, arrow_hits, arrow_cleanup).chain());
    }
}

/// Aim-in transition played when the player starts the bow attack.
const AIM_IN_ANIMATION: &str = "T_BOWRUN_2_BOWAIM";
/// Aim-out transition played when the player stops shooting.
const AIM_OUT_ANIMATION: &str = "T_BOWAIM_2_BOWRUN";
/// Delay between entering the aim stage and the first arrow release. The
/// aim/fire animation `S_BOWSHOOT` is a 1-frame pose the animation system
/// cannot advance (logged as "not playable"), so the hold is the looping
/// aim-in animation and the release is a fixed delay instead of an animation
/// frame.
const ARROW_RELEASE_DELAY: f32 = 0.3;
/// Interval between repeat shots while the attack key is held.
const SHOT_INTERVAL: f32 = 1.2;
/// Local axis of the arrow item mesh pointing along the shaft. The arrow
/// meshes are authored sideways (like weapons: grip at origin), not along -Z,
/// so the shot direction is aligned with this axis.
const ARROW_MESH_FORWARD: Vec3 = Vec3::X;
/// Initial arrow speed in meters per second.
const ARROW_SPEED: f32 = 30.0;
/// Height above the npc origin the arrow is shot from and aims at. The npc
/// capsule spans roughly -0.9 to +0.9 around the feet origin, so 0.9 is its
/// chest height.
const BOW_HEIGHT: f32 = 0.9;
/// Seconds after which an arrow that hit nothing despawns.
const ARROW_LIFETIME: f32 = 10.0;

/// Stages of the player bow attack sequence.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
enum BowStage {
    #[default]
    Idle,
    AimIn,
    Aim,
    AimOut,
}

/// State of the player bow attack sequence, on the player model entity.
#[derive(Component, Default)]
pub struct BowAttack {
    stage: BowStage,
    elapsed: f32,
    /// Time within the `Aim` stage at which the next arrow releases; `None`
    /// while no shot is scheduled.
    next_shot_at: Option<f32>,
}

/// An arrow about to be spawned by the bow attack of the given shooter.
#[derive(Message)]
pub struct ArrowShot {
    pub shooter: Entity,
}

/// Data of a flying arrow, used for damage and cleanup.
#[derive(Component)]
pub struct Arrow {
    pub damage: i32,
    pub shooter: InstanceId,
    spawned_at: f32,
}

/// Adds the bow attack state component to the player model entity, also after
/// the visual was respawned (far to near).
fn add_bow_attack(
    mut commands: Commands,
    players: Query<Entity, (Added<AnimationController>, With<PlayerCharacter>)>,
) {
    for entity in &players {
        commands.entity(entity).insert(BowAttack::default());
    }
}

/// Resolves the animation path for a bow animation of the player scheme.
fn bow_animation(
    model: &NpcModel,
    overlay: Option<&NpcOverlay>,
    files: &AnimationFiles,
    animation: &str,
    handles: &mut MaterialHandles,
    asset_server: &Res<AssetServer>,
) -> Handle<AnimationData> {
    let base = model.animation_scheme.as_deref().unwrap_or("HUMANS");
    let overlay = overlay.and_then(|overlay| overlay.0.as_deref());
    let scheme = choose_animation_scheme(base, overlay, animation, &files.0);
    let path = get_animation_path(scheme, animation);
    handles.get_animation_handle(asset_server, &path)
}

/// Duration of one animation loop in seconds, `None` while the asset loads or
/// for animations the playback system cannot advance.
fn animation_duration(
    animations: &Assets<AnimationData>,
    handle: &Handle<AnimationData>,
) -> Option<f32> {
    let data = animations.get(handle)?;
    if data.fps <= 0.0 || data.frames_num < 2 {
        return None;
    }
    Some(data.frames_num as f32 / data.fps)
}

/// Queues `handle` as the next animation unless it already plays.
fn queue_animation(
    controller: &mut AnimationController,
    handle: &Handle<AnimationData>,
    interrupt: bool,
) {
    if controller.current == *handle {
        return;
    }
    controller.next = Some(handle.clone());
    controller.interrupt_current = interrupt;
}

/// Advances the player bow attack sequence and queues the matching animations.
/// Runs between the locomotion animation pick and the animation advance, see
/// [`ScriptedAnimations`], so queued animations are applied in the same frame
/// and not overwritten while the current animation is locomotion-owned.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn bow_attack_state_machine(
    time: Res<Time>,
    action_state: Single<&ActionState<GameplayAction>, With<GameplayInputMarker>>,
    animations: Res<Assets<AnimationData>>,
    mut handles: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
    files: Res<AnimationFiles>,
    mut shots: MessageWriter<ArrowShot>,
    mut players: Query<
        (
            Entity,
            &NpcModel,
            Option<&NpcOverlay>,
            Option<&NpcWeaponMode>,
            Option<&LocomotionSequencer>,
            &mut AnimationController,
            &mut BowAttack,
        ),
        With<PlayerCharacter>,
    >,
) {
    let delta = time.delta_secs();
    let interact_held = action_state.pressed(&GameplayAction::Interact);

    for (entity, model, overlay, weapon_mode, sequencer, mut controller, mut bow) in &mut players {
        let mode = weapon_mode.map(|mode| mode.0).unwrap_or(WeaponMode::Fists);
        if mode != WeaponMode::Ranged && bow.stage != BowStage::Idle {
            // Weapon sheathed mid-sequence, cancel it. `apply_weapon_mode`
            // restores the idle on the weapon-mode change.
            info!("Bow attack of player({entity}) canceled, weapon no longer drawn");
            *bow = BowAttack::default();
            continue;
        }
        if mode != WeaponMode::Ranged {
            continue;
        }

        // Stage transitions queue their animation with `interrupt_current =
        // false`, so each animation plays to its end before the next one
        // starts; the stage only advances once the queued animation actually
        // became current and played for its duration. The exception is
        // stopping the aim hold, which interrupts immediately.
        match bow.stage {
            BowStage::Idle => {
                if !action_state.just_pressed(&GameplayAction::Interact) {
                    continue;
                }
                let aim_in = bow_animation(
                    model,
                    overlay,
                    &files,
                    AIM_IN_ANIMATION,
                    &mut handles,
                    &asset_server,
                );
                queue_animation(&mut controller, &aim_in, false);
                bow.stage = BowStage::AimIn;
                bow.elapsed = 0.0;
                bow.next_shot_at = None;
                info!("Player({entity}) starts a bow attack");
            }
            BowStage::AimIn => {
                let aim_in = bow_animation(
                    model,
                    overlay,
                    &files,
                    AIM_IN_ANIMATION,
                    &mut handles,
                    &asset_server,
                );
                queue_animation(&mut controller, &aim_in, false);
                // Wait until the queued animation actually plays, the previous
                // one has to end first.
                if controller.current != aim_in {
                    continue;
                }
                bow.elapsed += delta;
                let Some(duration) = animation_duration(&animations, &aim_in) else {
                    continue;
                };
                if bow.elapsed >= duration {
                    // The aim/fire animation is a 1-frame pose the playback
                    // cannot advance, so aiming keeps the aim-in animation
                    // looping and shots are timed instead.
                    bow.stage = BowStage::Aim;
                    bow.elapsed = 0.0;
                    // Holding the key fires the first shot after the release
                    // delay, further shots every [`SHOT_INTERVAL`].
                    bow.next_shot_at = interact_held.then_some(ARROW_RELEASE_DELAY);
                }
            }
            BowStage::Aim => {
                bow.elapsed += delta;
                if let Some(release) = bow.next_shot_at
                    && bow.elapsed >= release
                {
                    // A scheduled shot only fires while the key is still held.
                    bow.next_shot_at = if interact_held {
                        shots.write(ArrowShot { shooter: entity });
                        info!("Player({entity}) releases an arrow");
                        Some(bow.elapsed + SHOT_INTERVAL)
                    } else {
                        None
                    };
                }
                if bow.next_shot_at.is_none() && !interact_held {
                    let aim_out = bow_animation(
                        model,
                        overlay,
                        &files,
                        AIM_OUT_ANIMATION,
                        &mut handles,
                        &asset_server,
                    );
                    // The aim hold never ends on its own (1-frame animation),
                    // stop it immediately.
                    queue_animation(&mut controller, &aim_out, true);
                    bow.stage = BowStage::AimOut;
                    bow.elapsed = 0.0;
                }
            }
            BowStage::AimOut => {
                let aim_out = bow_animation(
                    model,
                    overlay,
                    &files,
                    AIM_OUT_ANIMATION,
                    &mut handles,
                    &asset_server,
                );
                // Re-queue every frame: a just-loaded animation handle can be
                // dropped by the playback while its asset is still loading.
                queue_animation(&mut controller, &aim_out, true);
                if controller.current != aim_out {
                    continue;
                }
                bow.elapsed += delta;
                let Some(duration) = animation_duration(&animations, &aim_out) else {
                    continue;
                };
                if bow.elapsed >= duration {
                    // Hand the animation control back to the locomotion idle;
                    // the armed idle `S_BOW` is a 1-frame pose the playback
                    // cannot switch to, the player would never leave the
                    // aim-out animation.
                    let idle = sequencer.map(|sequencer| sequencer.idle.clone());
                    if let Some(idle) = idle {
                        queue_animation(&mut controller, &idle, true);
                    }
                    *bow = BowAttack::default();
                    info!("Player({entity}) finished the bow attack");
                }
            }
        }
    }
}

/// Spawns an arrow entity for each requested shot, aimed at the selected npc
/// or straight ahead when nothing is selected.
#[allow(clippy::too_many_arguments)]
fn spawn_arrows(
    mut shots: MessageReader<ArrowShot>,
    mut commands: Commands,
    selection: Res<Selection>,
    players: Query<&GlobalTransform, With<PlayerCharacter>>,
    transforms: Query<&GlobalTransform>,
    npcs: Query<&GameNpc>,
    gothic_script: Res<ScriptResource>,
    vfs: Res<ZenginVfs>,
    mut handles: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) {
    for shot in shots.read() {
        let Ok(player_tr) = players.get(shot.shooter) else {
            warn!(
                "Arrow shot ignored: shooter({}) has no transform",
                shot.shooter
            );
            continue;
        };
        let Some(shooter) = npcs.get(shot.shooter).ok().map(|npc| npc.npc_instance_id) else {
            warn!(
                "Arrow shot ignored: shooter({}) is not an npc",
                shot.shooter
            );
            continue;
        };

        let origin = player_tr.translation() + Vec3::Y * BOW_HEIGHT;
        // Aim at the selected npc, fall back to the facing direction of the
        // player (it faces local +Z, see the selection module).
        let direction = selection
            .target
            .and_then(|target| transforms.get(target).ok())
            .map(|target| {
                (target.translation() + Vec3::Y * BOW_HEIGHT - origin).normalize_or_zero()
            })
            .filter(|direction| direction.length_squared() > 0.0)
            .unwrap_or_else(|| {
                let facing = player_tr.back();
                Vec3::new(facing.x, 0.0, facing.z).normalize_or_zero()
            });
        if direction.length_squared() < 0.5 {
            warn!(
                "Arrow of player({}) not spawned: no aim direction",
                shot.shooter
            );
            continue;
        }

        // Damage of the currently equipped ranged weapon.
        let damage = {
            let mut script = gothic_script.lock();
            script
                .npc_instances()
                .get(&shooter)
                .and_then(|player| player.equipped_ranged)
                .and_then(|weapon| script.item_damage_total(weapon))
        };
        let Some(damage) = damage else {
            warn!(
                "Arrow of player({}) not spawned: no ranged weapon damage",
                shot.shooter
            );
            continue;
        };

        // The arrow mesh is authored sideways (shaft along the local X axis,
        // like weapon meshes with the grip at the origin), so align that axis
        // with the shot direction instead of the default -Z look-at.
        let transform = Transform::from_translation(origin)
            .with_rotation(Quat::from_rotation_arc(ARROW_MESH_FORWARD, direction));
        let arrow = Arrow {
            damage,
            shooter,
            spawned_at: time.elapsed_secs(),
        };
        let entity = commands
            .spawn((
                arrow,
                RigidBody::Dynamic,
                Collider::cuboid(0.6, 0.05, 0.05),
                // The sensor collider contributes no mass, give the arrow an
                // explicit one so the rigid body has no NaN mass properties.
                MassPropertiesBundle::from_shape(&Collider::cuboid(0.6, 0.05, 0.05), 1.0),
                Sensor,
                CollisionEventsEnabled,
                // Fast small projectile, swept CCD avoids tunneling through
                // the npc capsules between the physics steps.
                SweptCcd::default(),
                LinearVelocity(direction * ARROW_SPEED),
                CollisionLayers::from_bits(ARROW, NPC_BODY),
                transform,
            ))
            .id();

        // Visual: the arrow item mesh when the ammo item instance exists,
        // otherwise a small visible box.
        let arrow_item = gothic_script
            .lock()
            .get_loaded_state()
            .items_instances
            .iter()
            .find(|(_, instance)| instance.name.eq_ignore_ascii_case("ITRW_ARROW"))
            .map(|(id, _)| *id);
        let visual = arrow_item.and_then(|item| item_model_path(&gothic_script, &vfs, item));
        match visual {
            Some(path) => {
                let model_handle = handles.get_model_handle(&asset_server, &path, None);
                commands.entity(entity).insert((
                    ZenGinModelComponent {
                        model_handle,
                        ..default()
                    },
                    Visibility::default(),
                ));
            }
            None => {
                commands.entity(entity).insert((
                    Mesh3d(meshes.add(Cuboid::new(0.6, 0.03, 0.03))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(0.6, 0.4, 0.2),
                        ..default()
                    })),
                ));
            }
        }
        info!(
            "Player({}) shot an arrow dealing {damage} damage towards {}",
            shooter.value(),
            selection
                .target
                .map(|target| format!("npc({target})"))
                .unwrap_or_else(|| "the facing direction".to_string())
        );
    }
}

/// Applies arrow damage on contact with an npc and despawns the arrow.
#[allow(clippy::too_many_arguments)]
fn arrow_hits(
    mut contacts: MessageReader<CollisionStart>,
    arrows: Query<&Arrow>,
    hit_colliders: Query<&NpcHitCollider>,
    npcs: Query<(&GameNpc, &GlobalTransform), Without<PlayerCharacter>>,
    gothic_script: Res<ScriptResource>,
    mut game_log: ResMut<GameLog>,
    mut npc_died: MessageWriter<NpcDied>,
    mut commands: Commands,
) {
    for contact in contacts.read() {
        let (arrow_entity, hit_entity) = if arrows.contains(contact.collider1) {
            (contact.collider1, contact.collider2)
        } else if arrows.contains(contact.collider2) {
            (contact.collider2, contact.collider1)
        } else {
            continue;
        };
        let Ok(arrow) = arrows.get(arrow_entity) else {
            continue;
        };
        // The npc hit collider is a child entity, resolve it to the npc. The
        // `Without<PlayerCharacter>` filter on the owner also keeps arrows from
        // hitting the shooter itself.
        let hit_entity = hit_colliders
            .get(hit_entity)
            .map(|collider| collider.owner)
            .unwrap_or(hit_entity);
        let Ok((npc, transform)) = npcs.get(hit_entity) else {
            continue;
        };
        let npc_id = npc.npc_instance_id;
        let hitpoints = gothic_script.lock().change_hitpoints(npc_id, -arrow.damage);
        let Some(hitpoints) = hitpoints else {
            warn!(
                "Arrow damage ignored: no hitpoints data for npc({})",
                npc_id.value()
            );
            continue;
        };
        info!(
            "Arrow of player({}) dealt {} damage to npc({}), hitpoints now {hitpoints}",
            arrow.shooter.value(),
            arrow.damage,
            npc_id.value()
        );
        game_log.add(format!("Dealt {} damage", arrow.damage));
        if hitpoints <= 0 {
            npc_died.write(NpcDied {
                entity: hit_entity,
                instance_id: npc_id,
                position: transform.translation(),
            });
        }
        commands.entity(arrow_entity).despawn();
    }
}

/// Despawns arrows that hit nothing after [`ARROW_LIFETIME`] seconds.
fn arrow_cleanup(time: Res<Time>, arrows: Query<(Entity, &Arrow)>, mut commands: Commands) {
    let now = time.elapsed_secs();
    for (entity, arrow) in &arrows {
        if now - arrow.spawned_at > ARROW_LIFETIME {
            commands.entity(entity).despawn();
        }
    }
}
