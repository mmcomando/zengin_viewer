//! Npc weapons: weapon mode idle switch and weapon mesh attachment.

use bevy::ecs::entity::Entities;
use bevy::prelude::*;

use crate::PlayerCharacter;
use crate::ZenginVfs;
use crate::common::{choose_animation_scheme, get_animation_path};
use crate::gameplay::objects::NpcModel;
use crate::loaders::vfs::item_model_path;
use crate::npc_actions::NpcItemUse;
use crate::npc_routines::overlay::{AnimationFiles, NpcOverlay};
use crate::npc_routines::routines::has_animation;
use crate::resources::{CachedAssetHandles, ZenGinModelComponent};
use crate::script::component::ScriptResource;
use crate::script::state::{WeaponClass, WeaponKind, WeaponMode};
use crate::script::symbol_id::InstanceId;
use crate::{AnimationController, AnimationTriggers};
use crate::{GameMob, NpcMobUse};
use dev_util::perf_debug;

/// Weapon mode of the npc (`Npc_SetToFightMode`/`SetToFistMode`/weapon AI),
/// maintained by the script bridge.
#[derive(Debug, Clone, Copy, Reflect, Component)]
pub struct NpcWeaponMode(pub WeaponMode);

/// Switches the idle animation of an npc when its weapon mode changes (drawn
/// melee uses the armed idle `S_1H`, otherwise the scheme standing idle). Also
/// applies once when the visual (animation controller) appears. The player is
/// excluded: its idle is handled by the human animation sequencer.
#[allow(clippy::type_complexity)]
pub fn apply_weapon_mode(
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
    let _perf = perf_debug!();
    for (model, overlay, weapon_mode, mut controller) in &mut npcs {
        let Some(scheme) = model.animation_scheme.as_deref() else {
            continue;
        };
        let animation = match weapon_mode.0 {
            WeaponMode::Melee => armed_idle(model, overlay, &files, "S_1H"),
            // Approximation: both ranged kinds use the bow idle, a per-kind
            // scheme choice needs the weapon kind, not only the mode
            WeaponMode::Ranged => armed_idle(model, overlay, &files, "S_BOW"),
            WeaponMode::Fists => model.stand_animation.clone(),
        };
        // No resolved standing idle, keep the current animation
        let Some(animation) = animation else {
            continue;
        };
        let overlay = overlay.and_then(|overlay| overlay.0.as_deref());
        let scheme = choose_animation_scheme(scheme, overlay, &animation, &files.0);
        let path = get_animation_path(scheme, &animation);
        controller.queue_next(handles.get_animation_handle(&asset_server, &path), true);
    }
}

/// Armed idle for the scheme when the archive has it (`S_1H`, `S_BOW`), the
/// scheme standing idle otherwise. Many creatures have no armed idle of their
/// own (e.g. `GOBBO` has no `S_1H`), so the missing file is not requested.
fn armed_idle(
    model: &NpcModel,
    overlay: Option<&NpcOverlay>,
    files: &AnimationFiles,
    armed: &str,
) -> Option<String> {
    if has_animation(model, overlay, files, armed) {
        return Some(armed.to_string());
    }
    model.stand_animation.clone()
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

    /// Slot entity by its skeleton node name, e.g. `ZS_RIGHTHAND`. Unknown
    /// names (mob slots like `ZS_SLOT`) resolve to `None`.
    fn slot_by_name(&self, name: &str) -> Option<Entity> {
        match name.to_uppercase().as_str() {
            "ZS_RIGHTHAND" => self.hand,
            "ZS_LEFTHAND" => self.left_hand,
            "ZS_SWORD" => self.sword,
            "ZS_LONGSWORD" => self.longsword,
            "ZS_BOW" => self.bow,
            "ZS_CROSSBOW" => self.crossbow,
            _ => None,
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

/// Equipped weapon visuals of an npc, managed by [`update_npc_attachments`].
#[derive(Component, Default, Clone, PartialEq)]
pub struct NpcAttachments {
    pub melee: NpcAttachment,
    pub ranged: NpcAttachment,
}

/// Marks an attachment visual child entity.
#[derive(Component)]
pub struct NpcAttachmentMarker;

/// Weapons equipped by the npc (`EquipItem`), maintained by the script bridge.
/// A melee and a ranged weapon can be equipped at the same time.
#[derive(Component, Clone)]
pub struct NpcEquippedWeapon {
    pub melee: Option<InstanceId>,
    pub ranged: Option<InstanceId>,
    /// Class of the equipped melee weapon, `None` when there is none.
    pub melee_class: Option<WeaponClass>,
}

type NpcAttachmentQueryData = (
    &'static NpcWeaponSlots,
    Option<&'static NpcEquippedWeapon>,
    Option<&'static NpcWeaponMode>,
    Option<&'static mut NpcAttachments>,
);

type ChangedWeaponAttachment = Or<(
    Changed<NpcEquippedWeapon>,
    Changed<NpcWeaponMode>,
    Changed<NpcWeaponSlots>,
)>;

/// Keeps a weapon mesh on each npc that has one equipped. Melee sheathes at the
/// hip, ranged on the back; switching to melee/ranged weapon mode moves the
/// weapon to the hand. Reacts to the bridge-synced equipped items and weapon
/// mode, and to the visual (weapon slots) appearing.
#[allow(clippy::too_many_arguments)]
pub fn update_npc_attachments(
    mut commands: Commands,
    gothic_scripts: Res<ScriptResource>,
    vfs: Res<ZenginVfs>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    entities: &Entities,
    candidates: Query<Entity, ChangedWeaponAttachment>,
    mut removed_equipped: RemovedComponents<NpcEquippedWeapon>,
    mut removed_mode: RemovedComponents<NpcWeaponMode>,
    mut npcs: Query<NpcAttachmentQueryData>,
) {
    let _perf = perf_debug!();
    let removed = removed_equipped.read().chain(removed_mode.read());
    for entity in candidates.iter().chain(removed) {
        let Ok((slots, equipped, mode, attachments)) = npcs.get_mut(entity) else {
            continue;
        };
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
            mode.map_or(WeaponMode::Fists, |mode| mode.0),
        );
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
    mode: WeaponMode,
) {
    // Work on a plain copy, written back at the end, so the component does not
    // need to exist yet
    let mut tracked = attachments.as_deref().cloned().unwrap_or_default();

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

    if attachments.as_deref() != Some(&tracked) {
        commands.entity(entity).try_insert(tracked);
    }
}

/// Items held in the npc's hands while it uses a mob: the mob's `useWithItem`
/// and items created by the mob-use animation events, managed by
/// [`update_npc_mob_use_items`].
#[derive(Component, Default, Clone, PartialEq)]
pub struct NpcMobUseItems {
    pub items: Vec<NpcAttachment>,
}

/// Item held in the npc's hand during an `AI_UseItemToState` pose, managed by
/// [`update_npc_item_use_items`].
#[derive(Component, Default, Clone, PartialEq)]
pub struct NpcItemUseItems {
    pub items: Vec<NpcAttachment>,
}

/// One item the mob-use transition places into a skeleton slot.
struct HeldItemRequest {
    item: String,
    slot: String,
}

/// Keeps the items the current mob use places into the npc's hands. The events
/// live on the mob-use transition animation (`T_<SCHEME>_S0_2_S<n>`), which the
/// viewer does not play, so they are applied when the npc attaches and dropped
/// when it releases the mob.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_npc_mob_use_items(
    mut commands: Commands,
    gothic_scripts: Res<ScriptResource>,
    vfs: Res<ZenginVfs>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    entities: &Entities,
    triggers: Res<AnimationTriggers>,
    candidates: Query<Entity, Or<(Changed<NpcMobUse>, Changed<NpcWeaponSlots>)>>,
    mut removed: RemovedComponents<NpcMobUse>,
    mobs: Query<&GameMob>,
    mut npcs: Query<(
        &NpcWeaponSlots,
        Option<&NpcMobUse>,
        Option<&mut NpcMobUseItems>,
    )>,
) {
    let _perf = perf_debug!();
    let removed = removed.read();
    for entity in candidates.iter().chain(removed) {
        let Ok((slots, mob_use, tracked)) = npcs.get_mut(entity) else {
            continue;
        };
        let requests = match mob_use {
            Some(mob_use) => mobs
                .get(mob_use.mob)
                .map_or_default(|mob| mob_use_held_items(&triggers, mob, mob_use)),
            None => Vec::new(),
        };
        sync_mob_use_items(
            &mut commands,
            entities,
            &gothic_scripts,
            &vfs,
            &mut handles,
            &asset_server,
            entity,
            slots,
            tracked,
            requests,
        );
    }
}

/// Transition animation whose events place the held items,
/// `T_<SCHEME>[_<POS>]_S0_2_S<n>`.
fn mob_use_transition_name(mob_use: &NpcMobUse) -> String {
    let scheme = mob_use.scheme.as_str().to_uppercase();
    let state = mob_use.state.max(1);
    match mob_use.position_tag.as_deref() {
        Some(tag) => format!("T_{scheme}_{tag}_S0_2_S{state}"),
        None => format!("T_{scheme}_S0_2_S{state}"),
    }
}

/// Items the mob-use transition places into the hands. `ITEM_INSERT` takes the
/// mob's `useWithItem`, `ITEM_CREATE`/`ITEM_EXCHANGE` take the event item.
fn mob_use_held_items(
    triggers: &AnimationTriggers,
    mob: &GameMob,
    mob_use: &NpcMobUse,
) -> Vec<HeldItemRequest> {
    let animation = mob_use_transition_name(mob_use);
    let mut requests: Vec<HeldItemRequest> = Vec::new();
    for event in triggers.item_events(&animation) {
        match event.kind.as_str() {
            "ITEM_INSERT" => {
                if !mob.use_with_item.is_empty() {
                    requests.retain(|request| request.slot != event.slot);
                    requests.push(HeldItemRequest {
                        item: mob.use_with_item.clone(),
                        slot: event.slot.clone(),
                    });
                }
            }
            "ITEM_CREATE" | "ITEM_EXCHANGE" => {
                if !event.item.is_empty() {
                    requests.retain(|request| request.slot != event.slot);
                    requests.push(HeldItemRequest {
                        item: event.item.clone(),
                        slot: event.slot.clone(),
                    });
                }
            }
            "ITEM_REMOVE" | "ITEM_DESTROY" => {
                requests.retain(|request| request.slot != event.slot);
            }
            _ => {}
        }
    }
    requests
}

/// Keeps the item of an active `AI_UseItemToState` pose in the hand slot its
/// entry animation inserts it into. The events live on
/// `T_<SCHEME>_STAND_2_S0`, which is played once; the item stays attached until
/// the pose is released.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_npc_item_use_items(
    mut commands: Commands,
    gothic_scripts: Res<ScriptResource>,
    vfs: Res<ZenginVfs>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    entities: &Entities,
    triggers: Res<AnimationTriggers>,
    candidates: Query<Entity, Or<(Changed<NpcItemUse>, Changed<NpcWeaponSlots>)>>,
    mut removed: RemovedComponents<NpcItemUse>,
    mut npcs: Query<(
        &NpcWeaponSlots,
        Option<&NpcItemUse>,
        Option<&mut NpcItemUseItems>,
    )>,
) {
    let _perf = perf_debug!();
    let removed = removed.read();
    for entity in candidates.iter().chain(removed) {
        let Ok((slots, item_use, tracked)) = npcs.get_mut(entity) else {
            continue;
        };
        let requests = item_use.map_or_else(Vec::new, |item_use| {
            item_use_held_items(&triggers, &gothic_scripts, item_use)
        });
        let previous = tracked.as_deref().cloned();

        let mut desired = Vec::new();
        for (item, slot_name) in requests {
            let Some(slot) = slots
                .slot_by_name(&slot_name)
                .filter(|slot| entities.contains(*slot))
            else {
                debug!(
                    "item use item({}) slot({slot_name}) is not available on the model",
                    item.value()
                );
                continue;
            };
            desired.push((item, slot));
        }

        let updated = NpcItemUseItems {
            items: sync_held_items(
                &mut commands,
                entities,
                &gothic_scripts,
                &vfs,
                &mut handles,
                &asset_server,
                previous
                    .as_ref()
                    .map_or_else(Vec::new, |tracked| tracked.items.clone()),
                desired,
            ),
        };
        if previous.as_ref() != Some(&updated) {
            commands.entity(entity).try_insert(updated);
        }
    }
}

/// Entry animation of an item pose, which carries the item slot events.
fn item_use_entry_name(item_use: &NpcItemUse) -> String {
    format!("T_{}_STAND_2_S0", item_use.scheme.as_str().to_uppercase())
}

/// Items the item-pose entry animation places into the hands. `ITEM_INSERT`
/// takes the used item, `ITEM_CREATE`/`ITEM_EXCHANGE` take the event item.
fn item_use_held_items(
    triggers: &AnimationTriggers,
    gothic_scripts: &ScriptResource,
    item_use: &NpcItemUse,
) -> Vec<(InstanceId, String)> {
    let animation = item_use_entry_name(item_use);
    let mut requests: Vec<(InstanceId, String)> = Vec::new();
    let mut script = gothic_scripts.lock();
    for event in triggers.item_events(&animation) {
        match event.kind.as_str() {
            "ITEM_INSERT" => {
                requests.retain(|(_, slot)| *slot != event.slot);
                requests.push((item_use.item, event.slot.clone()));
            }
            "ITEM_CREATE" | "ITEM_EXCHANGE" => {
                if event.item.is_empty() {
                    continue;
                }
                let Some(item) = script
                    .try_get_symbol_id(&event.item.to_lowercase())
                    .and_then(|symbol| InstanceId::new(symbol.value()))
                else {
                    warn!(
                        "item use item({}) is not declared in the scripts",
                        event.item
                    );
                    continue;
                };
                requests.retain(|(_, slot)| *slot != event.slot);
                requests.push((item, event.slot.clone()));
            }
            "ITEM_REMOVE" | "ITEM_DESTROY" => {
                requests.retain(|(_, slot)| *slot != event.slot);
            }
            _ => {}
        }
    }
    requests
}

/// Syncs the mob-use item children of one npc with the requested items/slots.
#[allow(clippy::too_many_arguments)]
fn sync_mob_use_items(
    commands: &mut Commands,
    entities: &Entities,
    gothic_scripts: &ScriptResource,
    vfs: &ZenginVfs,
    handles: &mut CachedAssetHandles,
    asset_server: &Res<AssetServer>,
    entity: Entity,
    slots: &NpcWeaponSlots,
    tracked: Option<Mut<'_, NpcMobUseItems>>,
    requests: Vec<HeldItemRequest>,
) {
    let previous = tracked.as_deref().cloned();

    // Resolve the requested items to instances and slots
    let mut desired: Vec<(InstanceId, Entity)> = Vec::new();
    {
        let mut script = gothic_scripts.lock();
        for request in requests {
            let Some(slot) = slots
                .slot_by_name(&request.slot)
                .filter(|slot| entities.contains(*slot))
            else {
                debug!(
                    "mob use item({}) slot({}) is not available on the model",
                    request.item, request.slot
                );
                continue;
            };
            let Some(symbol) = script.try_get_symbol_id(&request.item.to_lowercase()) else {
                warn!(
                    "mob use item({}) is not declared in the scripts",
                    request.item
                );
                continue;
            };
            let Some(item) = InstanceId::new(symbol.value()) else {
                continue;
            };
            desired.push((item, slot));
        }
    }

    let updated = NpcMobUseItems {
        items: sync_held_items(
            commands,
            entities,
            gothic_scripts,
            vfs,
            handles,
            asset_server,
            previous
                .as_ref()
                .map_or_else(Vec::new, |tracked| tracked.items.clone()),
            desired,
        ),
    };
    if previous.as_ref() != Some(&updated) {
        commands.entity(entity).try_insert(updated);
    }
}

/// Syncs the held-item attachment children of one npc with the desired
/// `(item, slot)` pairs. Reuses tracked attachments per slot and despawns the
/// rest.
#[allow(clippy::too_many_arguments)]
fn sync_held_items(
    commands: &mut Commands,
    entities: &Entities,
    gothic_scripts: &ScriptResource,
    vfs: &ZenginVfs,
    handles: &mut CachedAssetHandles,
    asset_server: &Res<AssetServer>,
    retained: Vec<NpcAttachment>,
    desired: Vec<(InstanceId, Entity)>,
) -> Vec<NpcAttachment> {
    // Drop children whose slot is no longer requested
    let mut remaining = Vec::new();
    for mut attachment in retained {
        if desired
            .iter()
            .any(|(_, slot)| attachment.slot == Some(*slot))
        {
            remaining.push(attachment);
        } else {
            detach_attachment(commands, entities, &mut attachment);
        }
    }

    // Create or update the requested children
    for (item, slot) in desired {
        let mut attachment = match remaining
            .iter()
            .position(|attachment| attachment.slot == Some(slot))
        {
            Some(index) => remaining.remove(index),
            None => NpcAttachment::default(),
        };
        update_attachment(
            commands,
            entities,
            &mut attachment,
            Some(item),
            Some(slot),
            gothic_scripts,
            vfs,
            handles,
            asset_server,
        );
        remaining.push(attachment);
    }

    remaining.sort_by_key(|attachment| attachment.slot);
    remaining
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn model(scheme: &str, stand: &str) -> NpcModel {
        NpcModel {
            animation_scheme: Some(scheme.to_string()),
            stand_animation: Some(stand.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn armed_idle_is_used_when_the_scheme_has_it() {
        let files = AnimationFiles(HashSet::from(["HUMANS-S_1H.MAN".to_string()]));

        let idle = armed_idle(&model("HUMANS", "S_RUN"), None, &files, "S_1H");

        assert_eq!(idle.as_deref(), Some("S_1H"));
    }

    #[test]
    fn missing_armed_idle_falls_back_to_the_standing_idle() {
        let files = AnimationFiles(HashSet::from(["GOBBO-S_FISTRUN.MAN".to_string()]));

        let idle = armed_idle(&model("GOBBO", "S_FISTRUN"), None, &files, "S_1H");

        assert_eq!(idle.as_deref(), Some("S_FISTRUN"));
    }
}
