//! Npc armor: replaces the body mesh with the equipped armor's skinned mesh and
//! restores the body when the armor is removed.

use bevy::ecs::system::SystemParam;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::prelude::*;

use crate::gameplay::objects::NpcModel;
use crate::zengin::ZenginVfs;
use crate::zengin::loaders::vfs::item_model_path;
use crate::zengin::resources::{CachedAssetHandles, ZenGinModelComponent};
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::symbol_id::InstanceId;
use crate::zengin::visual::model::ZenGinModel;

/// Armor equipped by the npc (`EquipItem`/`Mdl_SetVisualBody`), maintained by
/// the script bridge. Always present, `None` when no armor is equipped.
#[derive(Component, Clone, Copy)]
pub struct NpcArmorItem(pub Option<InstanceId>);

/// Body mesh child of an npc.
#[derive(Component)]
pub struct NpcBodyMarker;

/// Armor mesh child of an npc. Carries the data needed to build the skin while
/// the model asset is still loading; `ZenGinModelComponent` is added once the
/// model and the body skeleton are available.
#[derive(Component)]
pub(crate) struct ArmorVisual {
    wearer: Entity,
    item: InstanceId,
}

/// Armor whose mesh path could not be resolved, stops the materialize retry.
#[derive(Component)]
pub(crate) struct ArmorVisualFailedMarker;

/// Armor visuals still waiting for their model asset or body skeleton.
type PendingArmorVisuals<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static ArmorVisual),
    (
        Without<ZenGinModelComponent>,
        Without<ArmorVisualFailedMarker>,
    ),
>;

/// Asset resources needed to turn an [`ArmorVisual`] into a skinned mesh.
#[derive(SystemParam)]
pub(crate) struct ArmorAssets<'w> {
    gothic_scripts: Res<'w, ScriptResource>,
    vfs: Res<'w, ZenginVfs>,
    asset_server: Res<'w, AssetServer>,
    handles: ResMut<'w, CachedAssetHandles>,
    models: Res<'w, Assets<ZenGinModel>>,
    inverse_bindposes: ResMut<'w, Assets<SkinnedMeshInverseBindposes>>,
}

/// Keeps one armor visual child per npc in sync with [`NpcArmorItem`] and hides
/// the body mesh while armor is equipped. Also runs when the body reappears
/// (the visual children are despawned together with it at far distance).
pub(crate) fn sync_npc_armor_visual(
    mut commands: Commands,
    changed: Query<Entity, Changed<NpcArmorItem>>,
    new_bodies: Query<&ChildOf, Added<NpcBodyMarker>>,
    npcs: Query<&NpcArmorItem>,
    children: Query<&Children>,
    bodies: Query<(), With<NpcBodyMarker>>,
    armors: Query<&ArmorVisual>,
) {
    let new_body_parents = new_bodies.iter().map(ChildOf::parent);
    for entity in changed.iter().chain(new_body_parents) {
        let Ok(armor) = npcs.get(entity) else {
            continue;
        };
        reconcile_npc_armor(&mut commands, &children, &bodies, &armors, entity, armor.0);
    }
}

/// Builds the skinned armor mesh of every [`ArmorVisual`] whose model and body
/// skeleton are available. The model is loaded asynchronously, so entries that
/// are not ready yet are retried.
pub(crate) fn materialize_armor_visuals(
    mut commands: Commands,
    mut assets: ArmorAssets,
    armors: PendingArmorVisuals,
    npcs: Query<&NpcModel>,
    children: Query<&Children>,
    bodies: Query<&ZenGinModelComponent, With<NpcBodyMarker>>,
) {
    for (entity, armor) in &armors {
        let Ok(npc_model) = npcs.get(armor.wearer) else {
            continue;
        };
        let Some(wearer_children) = children.get(armor.wearer).ok() else {
            continue;
        };
        let Some(body) = wearer_children.iter().find(|child| bodies.contains(*child)) else {
            continue;
        };
        let Ok(body_model) = bodies.get(body) else {
            continue;
        };
        let Some(body_bones) = body_model.bones_data.as_ref() else {
            continue;
        };

        let Some(path) = item_model_path(&assets.gothic_scripts, &assets.vfs, armor.item) else {
            warn!("No armor mesh path for item({})", armor.item.value());
            commands.entity(entity).insert(ArmorVisualFailedMarker);
            continue;
        };
        let handle = assets.handles.get_model_handle(
            &assets.asset_server,
            &path,
            npc_model.hierarchy.as_deref(),
        );
        let Some(model) = assets.models.get(&handle) else {
            continue;
        };
        let inverse_bindposes = assets.handles.get_inverse_bindposes_handle(
            &mut assets.inverse_bindposes,
            model,
            &handle,
        );
        let joints = model
            .animation_bone_index
            .iter()
            .map(|index| body_bones.joints[*index])
            .collect();

        commands.entity(entity).insert(ZenGinModelComponent {
            model_handle: handle,
            bones_data: Some(SkinnedMesh {
                inverse_bindposes,
                joints,
            }),
            ..default()
        });
        debug!(
            "armor item({}) materialized on npc entity({:?})",
            armor.item.value(),
            armor.wearer
        );
    }
}

fn reconcile_npc_armor(
    commands: &mut Commands,
    children: &Query<&Children>,
    bodies: &Query<(), With<NpcBodyMarker>>,
    armors: &Query<&ArmorVisual>,
    entity: Entity,
    item: Option<InstanceId>,
) {
    let wearer_children = children.get(entity).ok();
    let body =
        wearer_children.and_then(|children| children.iter().find(|child| bodies.contains(*child)));
    let armor_child =
        wearer_children.and_then(|children| children.iter().find(|child| armors.contains(*child)));
    let current_item = armor_child
        .and_then(|child| armors.get(child).ok())
        .map(|armor| armor.item);

    if let Some(body) = body {
        let visibility = if item.is_some() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        commands.entity(body).insert(visibility);
    }

    if current_item == item {
        return;
    }
    if let Some(child) = armor_child {
        debug!("armor removed from npc entity({entity:?})");
        commands.entity(child).try_despawn();
    }

    let Some(item) = item else {
        return;
    };
    commands.spawn((
        Visibility::default(),
        Transform::IDENTITY,
        ChildOf(entity),
        ArmorVisual {
            wearer: entity,
            item,
        },
    ));
    debug!(
        "armor item({}) equipped on npc entity({entity:?})",
        item.value()
    );
}
