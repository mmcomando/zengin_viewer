//! Picks the world object (npc or item) the player is most likely aiming at and
//! draws its name above it, as a 3d text label anchored to the object.

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_rich_text3d::{Text3d, Text3dPlugin, Text3dStyling, TextAnchor, TextAtlas};

use crate::gameplay::character::PlayerPhysicsBodyMarker;
use crate::gameplay::objects::PlayerCharacter;
use crate::gui_debug::toggle_visibility::show_gizmos;
use crate::zengin::npc_routines::NpcWeaponMode;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::WeaponMode;
use crate::zengin::zengin_resources::{ARROW, NPC_BODY, SELECTABLE};
use crate::zengin::{GameItem, GameNpc};

/// Distance (meters) within which a world object can be selected.
const SELECTION_RANGE: f32 = 5.0;
/// Selection range with a ranged weapon drawn, matching the FAI long range.
const RANGED_SELECTION_RANGE: f32 = 20.0;
/// Radius of the hardcoded npc selection collider capsule.
const NPC_COLLIDER_RADIUS: f32 = 0.4;
/// Cylinder length of the hardcoded npc selection collider capsule, the caps
/// add twice the radius, so the total height is 1.8 m like a human npc.
const NPC_COLLIDER_LENGTH: f32 = 1.0;
/// Vertical offset of the npc collider above the npc entity origin. The npc
/// origin sits at the feet, so half of the capsule height places it exactly
/// over the body (0..1.8 m) instead of half-buried in the ground.
const NPC_COLLIDER_OFFSET: f32 = (NPC_COLLIDER_LENGTH + 2.0 * NPC_COLLIDER_RADIUS) * 0.5;
/// Height of the name label above a selected npc.
const NPC_LABEL_HEIGHT: f32 = 2.2;
/// Height of the name label above a selected item.
const ITEM_LABEL_HEIGHT: f32 = 0.6;
/// Base font size of the label in font units, scaled down to world units by
/// [`LABEL_WORLD_SCALE`] so the label is a few tenths of a meter tall.
const LABEL_FONT_SIZE: f32 = 64.0;
const LABEL_WORLD_SCALE: f32 = 0.25;

/// Kind of the currently selected world object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    Npc,
    Item,
}

/// The world object (npc or item) the player currently has selected, if any.
/// Objects in front of the player are preferred over closer ones behind it.
#[derive(Resource, Default)]
pub struct Selection {
    /// Selected entity, `None` when nothing is in range.
    pub target: Option<Entity>,
    /// Kind of the selected entity, `None` when nothing is selected.
    pub kind: Option<SelectionKind>,
    /// Display name drawn above the selected entity.
    pub name: String,
}

pub struct SelectionPlugin;

impl Plugin for SelectionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Selection>();
        // Text3dPlugin already registers TouchTextMaterial3dPlugin for
        // StandardMaterial, non-standard materials need it added manually.
        app.add_plugins(Text3dPlugin {
            load_system_fonts: true,
            ..default()
        });
        app.add_observer(on_selectable_added::<GameNpc>);
        app.add_observer(on_selectable_added::<GameItem>);
        app.add_systems(Startup, spawn_selection_label);
        app.add_systems(Update, (update_selection, update_selection_label).chain());
        app.add_systems(Update, draw_npc_colliders.run_if(show_gizmos));
    }
}

/// Marker of the 3d text label showing the selected name.
#[derive(Component)]
struct SelectionLabel;

/// Last text shown by the label, to only rebuild the text mesh on change.
#[derive(Component, Default)]
struct SelectionLabelText(String);

/// Marker of the npc hit collider child entity, pointing at the npc entity it
/// belongs to. The collider is a child because it is offset upwards from the
/// npc origin (which sits at the feet) while the npc entity transform itself
/// drives the visuals.
#[derive(Component)]
pub struct NpcHitCollider {
    pub owner: Entity,
}

/// Selection collider shape of a selectable world object.
trait SelectionCollider {
    fn selection_collider() -> Collider;
    /// Whether the kind is an npc (`GameNpc`), used for collision layers and
    /// gizmo rendering of the collider.
    fn is_npc() -> bool;
}

impl SelectionCollider for GameNpc {
    fn selection_collider() -> Collider {
        // Capsule roughly matching a human npc volume, used both for selection
        // and as the hit target of flying arrows (they collide with `NPC_BODY`).
        Collider::capsule(NPC_COLLIDER_RADIUS, NPC_COLLIDER_LENGTH)
    }

    fn is_npc() -> bool {
        true
    }
}

impl SelectionCollider for GameItem {
    fn selection_collider() -> Collider {
        Collider::cuboid(0.5, 0.5, 0.5)
    }

    fn is_npc() -> bool {
        false
    }
}

/// Gives every npc/item a collider so avian indexes it for the spatial query.
/// Npc colliders also carry `NPC_BODY` and accept contacts from the `ARROW`
/// layer so arrows can hit them; items remain query-only (they collide with
/// nothing). The npc collider is a child entity offset upwards from the feet
/// origin, see [`NpcHitCollider`]; the item collider stays on the item entity.
fn on_selectable_added<C: Component + SelectionCollider>(
    trigger: On<Add, C>,
    mut commands: Commands,
) {
    let (layers, filters) = if C::is_npc() {
        (SELECTABLE | NPC_BODY, ARROW)
    } else {
        (SELECTABLE, 0)
    };
    let collider = (
        C::selection_collider(),
        CollisionLayers::from_bits(layers, filters),
    );
    if C::is_npc() {
        commands.entity(trigger.entity).with_child((
            collider,
            NpcHitCollider {
                owner: trigger.entity,
            },
            Transform::from_xyz(0.0, NPC_COLLIDER_OFFSET, 0.0),
        ));
    } else {
        commands.entity(trigger.entity).insert(collider);
    }
}

/// Spawns the 3d text label, it is moved above the selected object every
/// frame. The text material uses the font atlas texture and no lighting so the
/// label reads the same everywhere.
fn spawn_selection_label(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    let material = materials.add(StandardMaterial {
        base_color_texture: Some(TextAtlas::DEFAULT_IMAGE.clone()),
        alpha_mode: AlphaMode::Mask(0.5),
        unlit: true,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        SelectionLabel,
        SelectionLabelText::default(),
        Text3d::new(""),
        Text3dStyling {
            size: LABEL_FONT_SIZE,
            font: "DejaVu Serif".into(),
            anchor: TextAnchor::BOTTOM_CENTER,
            color: Srgba::WHITE,
            stroke: None,
            world_scale: Some(Vec2::splat(LABEL_WORLD_SCALE)),
            ..default()
        },
        Mesh3d::default(),
        MeshMaterial3d(material),
        Visibility::Hidden,
    ));
}

fn keep_nearest(best: &mut Option<(Entity, f32)>, entity: Entity, distance: f32) {
    if best.is_none_or(|(_, best_distance)| distance < best_distance) {
        *best = Some((entity, distance));
    }
}

/// Selects the nearest in-range npc/item, preferring objects in front of the
/// player. With a weapon drawn only npcs are selectable; a drawn ranged weapon
/// extends the range to [`RANGED_SELECTION_RANGE`]. Candidate entities are
/// found with an avian spatial query over the AABBs of their selection
/// colliders, which must be on the queried entities themselves so the returned
/// entities match `GameNpc`/`GameItem`. The npc name is resolved through the
/// script only when the selection changes, so the per-frame cost is just the
/// query and the distance scan of its hits.
#[allow(clippy::type_complexity)]
fn update_selection(
    gothic_script: Res<ScriptResource>,
    mut selection: ResMut<Selection>,
    spatial_query: SpatialQuery,
    player: Query<(Entity, &GlobalTransform), With<PlayerPhysicsBodyMarker>>,
    player_weapon: Query<&NpcWeaponMode, With<PlayerCharacter>>,
    hit_colliders: Query<&NpcHitCollider>,
    candidates: Query<(
        Entity,
        Option<&GameNpc>,
        Option<&GameItem>,
        Has<PlayerCharacter>,
        &GlobalTransform,
    )>,
) {
    let Ok((player_entity, player_tr)) = player.single() else {
        return;
    };
    let weapon_mode = player_weapon
        .single()
        .map(|mode| mode.0)
        .unwrap_or(WeaponMode::Fists);
    let (range, npcs_only) = match weapon_mode {
        WeaponMode::Melee => (SELECTION_RANGE, true),
        WeaponMode::Ranged => (RANGED_SELECTION_RANGE, true),
        WeaponMode::Fists => (SELECTION_RANGE, false),
    };
    let player_pos = player_tr.translation();
    // The player faces +Z, see `face_towards` in the dialog module.
    let back = player_tr.back();
    let forward = Vec3::new(back.x, 0.0, back.z).normalize_or_zero();

    // `aabb_intersections_with_aabb` takes no filter, hits are filtered below.
    let query_aabb = Collider::sphere(range).aabb(player_pos, Quat::IDENTITY);
    let mut nearest: Option<(Entity, f32)> = None;
    let mut nearest_front: Option<(Entity, f32)> = None;

    for entity in spatial_query.aabb_intersections_with_aabb(query_aabb) {
        // Npc hit colliders are child entities, resolve them to the npc.
        let entity = hit_colliders
            .get(entity)
            .map(|collider| collider.owner)
            .unwrap_or(entity);
        // The player body collider is not selectable.
        if entity == player_entity {
            continue;
        }
        let Ok((_, npc, item, is_player, tr)) = candidates.get(entity) else {
            continue;
        };
        // The player visual carries `GameNpc` too, it is not selectable.
        if is_player || (npc.is_none() && item.is_none()) {
            continue;
        }
        // With a weapon drawn items are not selectable.
        if npcs_only && npc.is_none() {
            continue;
        }
        let delta = tr.translation() - player_pos;
        let distance = delta.length();
        if distance > range {
            continue;
        }
        let flat = Vec3::new(delta.x, 0.0, delta.z).normalize_or_zero();
        keep_nearest(&mut nearest, entity, distance);
        if forward.dot(flat) > 0.0 {
            keep_nearest(&mut nearest_front, entity, distance);
        }
    }

    let Some((entity, _)) = nearest_front.or(nearest) else {
        selection.target = None;
        selection.kind = None;
        selection.name.clear();
        return;
    };

    // Same target stays selected, its name is already resolved. An item stays
    // selected only while items can be selected at all.
    if selection.target == Some(entity)
        && (!npcs_only || selection.kind == Some(SelectionKind::Npc))
    {
        return;
    }

    let Ok((_, npc, item, _, _)) = candidates.get(entity) else {
        return;
    };
    if let Some(item) = item {
        selection.kind = Some(SelectionKind::Item);
        let raw = item.item_instance_id.value();
        selection.name = gothic_script
            .lock()
            .item_display_name(item.item_instance_id)
            .or_else(|| gothic_script.lock().get_name_by_id(raw))
            .unwrap_or_else(|| raw.to_string());
    } else if let Some(npc) = npc {
        selection.kind = Some(SelectionKind::Npc);
        selection.name = gothic_script
            .lock()
            .npc_display_name(npc.npc_instance_id)
            .unwrap_or_else(|| {
                gothic_script
                    .lock()
                    .call_get_name(npc.npc_instance_id.symbol())
            });
    }
    selection.target = Some(entity);
}

/// Places the label above the selected object's name anchor, facing the
/// camera, and hides it when nothing is selected.
fn update_selection_label(
    selection: Res<Selection>,
    camera: Query<&GlobalTransform, With<Camera3d>>,
    targets: Query<&GlobalTransform>,
    mut label: Query<
        (
            &mut Text3d,
            &mut SelectionLabelText,
            &mut Transform,
            &mut Visibility,
        ),
        With<SelectionLabel>,
    >,
) {
    let Ok((mut text3d, mut label_text, mut transform, mut visibility)) = label.single_mut() else {
        return;
    };
    let Some(target) = selection.target.and_then(|entity| targets.get(entity).ok()) else {
        *visibility = Visibility::Hidden;
        return;
    };
    let Ok(camera_tr) = camera.single() else {
        *visibility = Visibility::Hidden;
        return;
    };

    let height = match selection.kind {
        Some(SelectionKind::Item) => ITEM_LABEL_HEIGHT,
        Some(SelectionKind::Npc) | None => NPC_LABEL_HEIGHT,
    };
    transform.translation = target.translation() + Vec3::Y * height;
    // Billboard: the text mesh faces its local +Z, so aligning it with the
    // camera orientation makes the label always readable.
    transform.rotation = camera_tr.rotation();

    if label_text.0 != selection.name {
        *text3d = Text3d::new(selection.name.clone());
        label_text.0.clone_from(&selection.name);
    }
    *visibility = Visibility::Inherited;
}

/// Draws the npc collider capsules as debug gizmos when the "Show Gizmos"
/// toggle is on, so the selection/arrow hit volume can be verified visually.
/// The colliders are child entities offset above the feet origin, the gizmo is
/// drawn at the child transform, which already includes the offset.
fn draw_npc_colliders(
    mut gizmos: Gizmos,
    npcs: Query<(&GlobalTransform, &NpcHitCollider)>,
    players: Query<(), With<PlayerCharacter>>,
) {
    let capsule = Capsule3d::new(NPC_COLLIDER_RADIUS, NPC_COLLIDER_LENGTH);
    for (transform, collider) in &npcs {
        if players.contains(collider.owner) {
            continue;
        }
        gizmos.primitive_3d(
            &capsule,
            transform.translation(),
            Color::srgb(1.0, 0.2, 0.2),
        );
    }
}
