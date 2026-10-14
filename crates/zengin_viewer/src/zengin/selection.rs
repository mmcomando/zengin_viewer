//! Picks the world object (npc or item) the player is most likely aiming at and
//! draws its name above it, as a screen-space UI label anchored to the object.

use bevy::prelude::*;

use crate::gameplay::character::PlayerPhysicsBodyMarker;
use crate::gameplay::objects::PlayerCharacter;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::{GameItem, GameNpc};

/// Distance (meters) within which a world object can be selected.
const SELECTION_RANGE: f32 = 5.0;
/// Height of the name label above a selected npc.
const NPC_LABEL_HEIGHT: f32 = 2.0;
/// Height of the name label above a selected item.
const ITEM_LABEL_HEIGHT: f32 = 0.6;
/// Font size of the label in logical pixels.
const LABEL_FONT_SIZE: f32 = 18.0;
/// Draw order of the label, above the dialog UI.
const LABEL_Z_INDEX: i32 = 10;

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
        app.add_systems(Startup, spawn_selection_label);
        app.add_systems(Update, (update_selection, update_selection_label).chain());
    }
}

/// Marker for the screen-space text node showing the selected name.
#[derive(Component)]
struct SelectionLabel;

/// Spawns the label as an absolutely positioned UI node, it is moved to the
/// projected screen position of the selected object every frame.
fn spawn_selection_label(mut commands: Commands) {
    commands.spawn((
        SelectionLabel,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            ..default()
        },
        Text::new(""),
        TextFont {
            font_size: LABEL_FONT_SIZE.into(),
            ..default()
        },
        TextColor(Color::WHITE),
        TextShadow {
            offset: Vec2::new(1.0, 1.0),
            color: Color::BLACK,
        },
        Visibility::Hidden,
        ZIndex(LABEL_Z_INDEX),
    ));
}

fn keep_nearest(best: &mut Option<(Entity, f32)>, entity: Entity, distance: f32) {
    if best.is_none_or(|(_, best_distance)| distance < best_distance) {
        *best = Some((entity, distance));
    }
}

/// Selects the nearest in-range npc/item, preferring objects in front of the
/// player. The npc name is resolved through the script only when the selection
/// changes, so the per-frame cost is just the distance scan.
#[allow(clippy::type_complexity)]
fn update_selection(
    gothic_script: Res<ScriptResource>,
    mut selection: ResMut<Selection>,
    player: Query<&GlobalTransform, With<PlayerPhysicsBodyMarker>>,
    candidates: Query<
        (
            Entity,
            Option<&GameNpc>,
            Option<&GameItem>,
            Has<PlayerCharacter>,
            &GlobalTransform,
        ),
        Or<(With<GameNpc>, With<GameItem>)>,
    >,
) {
    let Ok(player_tr) = player.single() else {
        return;
    };
    let player_pos = player_tr.translation();
    // The player faces +Z, see `face_towards` in the dialog module.
    let back = player_tr.back();
    let forward = Vec3::new(back.x, 0.0, back.z).normalize_or_zero();

    let mut nearest: Option<(Entity, f32)> = None;
    let mut nearest_front: Option<(Entity, f32)> = None;

    for (entity, npc, item, is_player, tr) in &candidates {
        // The player visual carries `GameNpc` too, it is not selectable.
        if is_player || (npc.is_none() && item.is_none()) {
            continue;
        }
        let delta = tr.translation() - player_pos;
        let distance = delta.length();
        if distance > SELECTION_RANGE {
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

    // Same target stays selected, its name is already resolved.
    if selection.target == Some(entity) {
        return;
    }

    let Ok((_, npc, item, _, _)) = candidates.get(entity) else {
        return;
    };
    if let Some(item) = item {
        selection.kind = Some(SelectionKind::Item);
        selection.name.clone_from(&item.name);
    } else if let Some(npc) = npc {
        selection.kind = Some(SelectionKind::Npc);
        selection.name = gothic_script
            .lock()
            .call_get_name(npc.npc_instance_id.symbol());
    }
    selection.target = Some(entity);
}

/// Places the label at the screen position the selected object's name anchor
/// projects to, and hides it when nothing is selected or the anchor is off
/// screen (behind the camera).
fn update_selection_label(
    selection: Res<Selection>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    targets: Query<&GlobalTransform>,
    mut label: Query<(&mut Node, &mut Text, &mut Visibility, &ComputedNode), With<SelectionLabel>>,
) {
    let Ok((mut node, mut text, mut visibility, computed)) = label.single_mut() else {
        return;
    };
    let Some(target) = selection.target.and_then(|entity| targets.get(entity).ok()) else {
        *visibility = Visibility::Hidden;
        return;
    };
    let Ok((camera, camera_tr)) = camera.single() else {
        *visibility = Visibility::Hidden;
        return;
    };

    let height = match selection.kind {
        Some(SelectionKind::Item) => ITEM_LABEL_HEIGHT,
        Some(SelectionKind::Npc) | None => NPC_LABEL_HEIGHT,
    };
    let anchor = target.translation() + Vec3::Y * height;
    // `world_to_viewport` fails for points behind the camera.
    let Ok(screen) = camera.world_to_viewport(camera_tr, anchor) else {
        *visibility = Visibility::Hidden;
        return;
    };

    // Center the label horizontally, put its bottom at the anchor (the size is
    // the previous frame's, which is fine for a static name).
    let size = computed.size();
    node.left = Val::Px(screen.x - size.x * 0.5);
    node.top = Val::Px(screen.y - size.y);

    if text.0 != selection.name {
        text.0.clone_from(&selection.name);
    }
    *visibility = Visibility::Inherited;
}
