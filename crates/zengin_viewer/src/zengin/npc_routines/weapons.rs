//! Npc weapons: weapon mode idle switch and weapon mesh attachment.

use std::collections::HashSet;

use bevy::ecs::entity::Entities;
use bevy::prelude::*;

use crate::gameplay::objects::{NpcModel, PlayerCharacter};
use crate::graphics::AnimationController;
use crate::zengin::ZenginVfs;
use crate::zengin::common::{choose_animation_scheme, get_animation_path};
use crate::zengin::loaders::vfs::item_model_path;
use crate::zengin::npc_routines::overlay::{AnimationFiles, NpcOverlay};
use crate::zengin::resources::{CachedAssetHandles, ZenGinModelComponent};
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::{NpcEntitiesChanged, WeaponKind, WeaponMode};
use crate::zengin::script::symbol_id::InstanceId;

/// Weapon mode of the npc (`Npc_SetToFightMode`/`SetToFistMode`/weapon AI),
/// maintained by the script bridge.
#[derive(Component)]
pub struct NpcWeaponMode(pub WeaponMode);

/// Switches the idle animation of an npc when its weapon mode changes (drawn
/// melee uses the armed idle `S_1H`, otherwise the scheme standing idle). Also
/// applies once when the visual (animation controller) appears. The player is
/// excluded: its idle is handled by the human animation sequencer.
#[allow(clippy::type_complexity)]
pub(crate) fn apply_weapon_mode(
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut npcs: Query<
        (
            &NpcModel,
            Option<&NpcOverlay>,
            &NpcWeaponMode,
            &mut AnimationController,
        ),
        (
            Or<(Changed<NpcWeaponMode>, Added<AnimationController>)>,
            Without<PlayerCharacter>,
        ),
    >,
) {
    for (model, overlay, weapon_mode, mut controller) in &mut npcs {
        let Some(scheme) = model.animation_scheme.as_deref() else {
            continue;
        };
        let animation = match weapon_mode.0 {
            WeaponMode::Melee => "S_1H".to_string(),
            // Approximation: both ranged kinds use the bow idle, a per-kind
            // scheme choice needs the weapon kind, not only the mode
            WeaponMode::Ranged => "S_BOW".to_string(),
            WeaponMode::Fists => {
                // No resolved standing idle, keep the current animation
                let Some(stand) = model.stand_animation.clone() else {
                    continue;
                };
                stand
            }
        };
        let overlay = overlay.and_then(|overlay| overlay.0.as_deref());
        let scheme = choose_animation_scheme(scheme, overlay, &animation, &files.0);
        let path = get_animation_path(scheme, &animation);
        controller.queue_next(handles.get_animation_handle(&asset_server, &path), true);
    }
}

/// Engine weapon slot nodes of an npc model. Melee weapons sheathe at
/// `sword`/`longsword` (hip), ranged at `bow`/`crossbow` (back); drawn weapons
/// go to the hands (bow in the left hand, everything else in the right).
#[derive(Component, Default)]
pub struct NpcWeaponSlots {
    pub hand: Option<Entity>,
    pub left_hand: Option<Entity>,
    pub sword: Option<Entity>,
    pub longsword: Option<Entity>,
    pub bow: Option<Entity>,
    pub crossbow: Option<Entity>,
}

impl NpcWeaponSlots {
    /// Slot a weapon of `kind` attaches to when `drawn`, `None` when the model
    /// has no matching node.
    fn slot_for(&self, kind: WeaponKind, drawn: bool) -> Option<Entity> {
        match (kind, drawn) {
            (WeaponKind::Melee | WeaponKind::Crossbow, true) => self.hand,
            (WeaponKind::Melee, false) => self.sword.or(self.longsword),
            (WeaponKind::Bow, true) => self.left_hand,
            (WeaponKind::Bow, false) => self.bow,
            (WeaponKind::Crossbow, false) => self.crossbow,
        }
    }
}

/// One attachment slot of an npc: the managed child entity and the source
/// values it was last built from.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct NpcAttachment {
    pub entity: Option<Entity>,
    pub item: Option<InstanceId>,
    pub slot: Option<Entity>,
}

/// Equipped weapon (and later armor) visuals of an npc, managed by
/// [`update_npc_attachments`].
#[derive(Component, Default, Clone, PartialEq)]
pub struct NpcAttachments {
    pub melee: NpcAttachment,
    pub ranged: NpcAttachment,
    /// Reserved for the armor replacement, currently produced by the spawn path.
    pub armor: NpcAttachment,
    /// Weapon mode the slots were last laid out for.
    pub mode: Option<WeaponMode>,
}

/// Marks an attachment visual child entity.
#[derive(Component)]
pub struct NpcAttachmentMarker;

/// Item instance rendered by an attachment child.
#[derive(Component, Clone, Copy)]
#[allow(dead_code)] // no item consumers yet
pub struct EquippedItem {
    pub instance_id: InstanceId,
}

/// Weapons equipped by the npc (`EquipItem`), maintained by the script bridge.
/// A melee and a ranged weapon can be equipped at the same time.
#[derive(Component, Clone)]
pub struct NpcEquippedWeapon {
    pub melee: Option<InstanceId>,
    pub ranged: Option<InstanceId>,
}

type NpcAttachmentQueryData = (
    Entity,
    &'static NpcWeaponSlots,
    Option<&'static NpcEquippedWeapon>,
    Option<&'static NpcWeaponMode>,
    Option<&'static mut NpcAttachments>,
);

/// Keeps a weapon mesh on each npc that has one equipped. Melee sheathes at the
/// hip, ranged on the back; switching to melee/ranged weapon mode moves the
/// weapon to the hand. Reacts to the npc change signal, the equipped items
/// changing, the weapon mode changing, and the visual (weapon slots) appearing.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn update_npc_attachments(
    mut commands: Commands,
    mut changes: MessageReader<NpcEntitiesChanged>,
    gothic_scripts: Res<ScriptResource>,
    vfs: Res<ZenginVfs>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    entities: &Entities,
    mut npcs: ParamSet<(
        Query<NpcAttachmentQueryData>,
        Query<
            NpcAttachmentQueryData,
            Or<(
                Changed<NpcEquippedWeapon>,
                Changed<NpcWeaponMode>,
                Changed<NpcWeaponSlots>,
            )>,
        >,
    )>,
) {
    {
        let mut processed = HashSet::new();
        {
            let mut query = npcs.p0();
            for change in changes.read() {
                let (melee, ranged, mode) = {
                    let runtime = change
                        .handle
                        .runtime()
                        .read()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    (
                        runtime.equipment.melee,
                        runtime.equipment.ranged,
                        runtime.equipment.weapon_mode,
                    )
                };
                for &entity in &change.entities {
                    let Ok((entity, slots, _, _, attachments)) = query.get_mut(entity) else {
                        continue;
                    };
                    processed.insert(entity);
                    sync_npc_attachments(
                        &mut commands,
                        entities,
                        &gothic_scripts,
                        &vfs,
                        &mut handles,
                        &asset_server,
                        entity,
                        slots,
                        attachments,
                        melee,
                        ranged,
                        mode,
                    );
                }
            }
        }

        let mut query = npcs.p1();
        for (entity, slots, equipped, mode, attachments) in &mut query {
            if processed.contains(&entity) {
                continue;
            }
            sync_npc_attachments(
                &mut commands,
                entities,
                &gothic_scripts,
                &vfs,
                &mut handles,
                &asset_server,
                entity,
                slots,
                attachments,
                equipped.and_then(|equipped| equipped.melee),
                equipped.and_then(|equipped| equipped.ranged),
                mode.map(|mode| mode.0),
            );
        }
    }
}

/// Syncs the melee and ranged attachment children of one npc with the equipped
/// items, the weapon mode and the available skeleton slots.
#[allow(clippy::too_many_arguments)]
fn sync_npc_attachments(
    commands: &mut Commands,
    entities: &Entities,
    gothic_scripts: &ScriptResource,
    vfs: &ZenginVfs,
    handles: &mut CachedAssetHandles,
    asset_server: &Res<AssetServer>,
    entity: Entity,
    slots: &NpcWeaponSlots,
    attachments: Option<Mut<'_, NpcAttachments>>,
    melee_item: Option<InstanceId>,
    ranged_item: Option<InstanceId>,
    mode: Option<WeaponMode>,
) {
    let mode = mode.unwrap_or(WeaponMode::Fists);
    // Work on a plain copy, written back at the end, so the component does not
    // need to exist yet
    let mut tracked = attachments.as_deref().cloned().unwrap_or_default();
    let child_missing = tracked
        .melee
        .entity
        .is_some_and(|child| !entities.contains(child))
        || tracked
            .ranged
            .entity
            .is_some_and(|child| !entities.contains(child));
    if tracked.melee.item == melee_item
        && tracked.ranged.item == ranged_item
        && tracked.mode == Some(mode)
        && !child_missing
    {
        return;
    }

    // Bow and crossbow share the equipped ranged item; both `ZS_` slots exist
    // on humanoid models, so the kind is resolved from the item's symbol name,
    // not from the slot the model provides
    let ranged_kind = ranged_item
        .and_then(|item| {
            let mut script = gothic_scripts.lock();
            script.get_name_by_id(item.value())
        })
        .map(|name| name.to_lowercase())
        .and_then(|name| WeaponKind::from_symbol_name(&name))
        .filter(|kind| *kind != WeaponKind::Melee)
        .unwrap_or(WeaponKind::Bow);

    for (kind, item, attachment) in [
        (WeaponKind::Melee, melee_item, &mut tracked.melee),
        (ranged_kind, ranged_item, &mut tracked.ranged),
    ] {
        let drawn = match kind {
            WeaponKind::Melee => mode == WeaponMode::Melee,
            WeaponKind::Bow | WeaponKind::Crossbow => mode == WeaponMode::Ranged,
        };
        let target_slot = slots
            .slot_for(kind, drawn)
            .filter(|slot| entities.contains(*slot));
        update_attachment(
            commands,
            entities,
            attachment,
            item,
            target_slot,
            gothic_scripts,
            vfs,
            handles,
            asset_server,
        );
    }

    tracked.mode = Some(mode);
    if attachments.as_deref() != Some(&tracked) {
        commands.entity(entity).try_insert(tracked);
    }
}

/// Syncs one attachment child with its equipped item and target slot: creates
/// the mesh child when missing or the item changed, moves it between slots and
/// despawns it when nothing is equipped.
#[allow(clippy::too_many_arguments)]
fn update_attachment(
    commands: &mut Commands,
    entities: &Entities,
    attachment: &mut NpcAttachment,
    item: Option<InstanceId>,
    target_slot: Option<Entity>,
    gothic_scripts: &ScriptResource,
    vfs: &ZenginVfs,
    handles: &mut CachedAssetHandles,
    asset_server: &Res<AssetServer>,
) {
    let Some(item) = item else {
        detach_attachment(commands, entities, attachment);
        return;
    };

    // The attachment child is despawned with the visual when the npc is far
    let child_alive = attachment
        .entity
        .is_some_and(|child| entities.contains(child));
    // A different equipped item needs a different mesh child
    let item_changed = attachment.item != Some(item);
    let slot_changed = attachment.slot != target_slot;
    if child_alive && !item_changed && !slot_changed {
        return;
    }

    let Some(path) = item_model_path(gothic_scripts, vfs, item) else {
        // Nothing equipped, or its mesh is unknown
        detach_attachment(commands, entities, attachment);
        return;
    };

    if !child_alive || item_changed {
        // Replace the stale/obsolete attachment child
        if let Some(child) = attachment.entity.filter(|child| entities.contains(*child)) {
            commands.entity(child).despawn();
        }
        let Some(target_slot) = target_slot else {
            // No slot available, only drop the stale child reference
            attachment.entity = None;
            attachment.slot = None;
            return;
        };
        let handle = handles.get_model_handle(asset_server, &path, None);
        let child = commands
            .spawn((
                Visibility::default(),
                ZenGinModelComponent {
                    model_handle: handle,
                    ..default()
                },
                Transform::IDENTITY,
                ChildOf(target_slot),
                NpcAttachmentMarker,
                EquippedItem { instance_id: item },
            ))
            .id();
        debug!(
            "attachment item({}) created as child({child:?}) of slot({target_slot:?})",
            item.value()
        );
        *attachment = NpcAttachment {
            entity: Some(child),
            item: Some(item),
            slot: Some(target_slot),
        };
        return;
    }

    attachment.item = Some(item);

    // Move between sheathed and drawn
    if let Some(target_slot) = target_slot
        && attachment.slot != Some(target_slot)
        && let Some(child) = attachment.entity
    {
        debug!(
            "attachment item({:?}) moved to slot({target_slot:?})",
            attachment.item
        );
        commands.entity(child).insert(ChildOf(target_slot));
        attachment.slot = Some(target_slot);
    }
}

/// Despawns the attachment child when it is still alive and clears the tracked
/// state.
fn detach_attachment(commands: &mut Commands, entities: &Entities, attachment: &mut NpcAttachment) {
    if let Some(child) = attachment.entity.take()
        && entities.contains(child)
    {
        debug!("attachment item({:?}) detached", attachment.item);
        commands.entity(child).despawn();
    }
    attachment.item = None;
    attachment.slot = None;
}
