// TODO Split to submodule "interaction"
//! Ranged combat: arrow projectiles of the player bow attack.
//!
//!  This module spawns the flying arrows, resolves their hits and cleans them up.

use bevy::prelude::*;

use crate::gameplay::objects::{NpcModel, PlayerCharacter};
use crate::graphics::AnimationController;
use crate::gui::GameLog;
use crate::zengin::interaction::{NpcDamageData, NpcDied, NpcHitReaction, damage_npc};
use crate::zengin::npc_routines::{AnimationFiles, NpcOverlay};
use crate::zengin::script::component::ScriptResource;
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
        app.add_systems(Update, (spawn_arrows, arrow_hits, arrow_cleanup).chain());
    }
}

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
    mut npcs: Query<
        (
            &GameNpc,
            &GlobalTransform,
            &NpcModel,
            Option<&NpcOverlay>,
            Option<&mut AnimationController>,
            Option<&NpcHitReaction>,
        ),
        Without<PlayerCharacter>,
    >,
    gothic_script: Res<ScriptResource>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<MaterialHandles>,
    asset_server: Res<AssetServer>,
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
        // TODO ignore shooter by adding minimal distance between arrow and hit.
        // The npc hit collider is a child entity, resolve it to the npc. The
        // `Without<PlayerCharacter>` filter on the owner also keeps arrows from
        // hitting the shooter itself.
        let hit_entity = hit_colliders
            .get(hit_entity)
            .map(|collider| collider.owner)
            .unwrap_or(hit_entity);
        let Ok((npc, global_tr, npc_model, overlay, controller, reaction)) =
            npcs.get_mut(hit_entity)
        else {
            continue;
        };
        let npc_data = NpcDamageData {
            npc,
            global_tr,
            model: npc_model,
            overlay,
            controller,
            reaction,
        };
        damage_npc(
            &mut commands,
            hit_entity,
            npc_data,
            arrow.damage,
            arrow.shooter,
            &gothic_script,
            &files,
            &mut handles,
            &asset_server,
            &mut game_log,
            &mut npc_died,
        );
        commands.entity(arrow_entity).despawn();
    }
}

/// Despawns arrows that hit nothing after [`ARROW_LIFETIME`] seconds.
fn arrow_cleanup(time: Res<Time>, arrows: Query<(Entity, &Arrow)>, mut commands: Commands) {
    let now = time.elapsed_secs();
    for (entity, arrow) in &arrows {
        if now - arrow.spawned_at > ARROW_LIFETIME {
            commands.entity(entity).try_despawn();
        }
    }
}
