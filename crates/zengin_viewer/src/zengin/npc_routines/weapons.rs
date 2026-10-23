//! Npc weapons: weapon mode idle switch and weapon mesh attachment.

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
use crate::zengin::script::state::{WeaponKind, WeaponMode};
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
            (WeaponKind::Melee, true) => self.hand,
            (WeaponKind::Melee, false) => self.sword.or(self.longsword),
            (WeaponKind::Bow, true) => self.left_hand,
            (WeaponKind::Bow, false) => self.bow,
            (WeaponKind::Crossbow, true) => self.hand,
            (WeaponKind::Crossbow, false) => self.crossbow,
        }
    }
}

/// One attached weapon mesh child of an npc, managed by [`update_npc_weapon`].
#[derive(Default, Clone, PartialEq)]
struct WeaponMesh {
    entity: Option<Entity>,
    item: Option<InstanceId>,
    slot: Option<Entity>,
}

/// Weapon mesh children currently attached to an npc per weapon kind, managed
/// by [`update_npc_weapon`].
#[derive(Component, Default, Clone, PartialEq)]
pub(crate) struct NpcWeapon {
    melee: WeaponMesh,
    ranged: WeaponMesh,
}

/// Weapons equipped by the npc (`EquipItem`), maintained by the script bridge.
/// A melee and a ranged weapon can be equipped at the same time.
#[derive(Component, Clone)]
pub struct NpcEquippedWeapon {
    pub melee: Option<InstanceId>,
    pub ranged: Option<InstanceId>,
}

/// Keeps a weapon mesh on each npc that has one equipped. Melee sheathes at the
/// hip, ranged on the back; switching to melee/ranged weapon mode moves the
/// weapon to the hand. Reacts to the equipped items changing, the weapon mode
/// changing, and the visual (weapon slots) appearing.
#[allow(clippy::type_complexity)]
pub(crate) fn update_npc_weapon(
    gothic_scripts: Res<ScriptResource>,
    vfs: Res<ZenginVfs>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    entities: &Entities,
    mut npcs: Query<
        (
            Entity,
            &NpcWeaponSlots,
            Option<&NpcEquippedWeapon>,
            Option<&NpcWeaponMode>,
            Option<&mut NpcWeapon>,
        ),
        Or<(
            Changed<NpcEquippedWeapon>,
            Changed<NpcWeaponMode>,
            Changed<NpcWeaponSlots>,
        )>,
    >,
) {
    for (entity, slots, equipped, weapon_mode, weapon) in &mut npcs {
        let mode = weapon_mode.map_or(WeaponMode::Fists, |mode| mode.0);
        let (melee_item, ranged_item) = equipped
            .map(|equipped| (equipped.melee, equipped.ranged))
            .unwrap_or_default();

        // Work on a plain copy, written back at the end, so the component does
        // not need to exist yet
        let mut tracked = weapon.as_deref().cloned().unwrap_or_default();
        // Bow and crossbow share the equipped ranged item; both `ZS_` slots
        // exist on humanoid models, so the kind is resolved from the item's
        // symbol name, not from the slot the model provides
        let ranged_kind = ranged_item
            .and_then(|item| {
                let mut script = gothic_scripts.lock();
                script.get_name_by_id(item.value())
            })
            .map(|name| name.to_lowercase())
            .and_then(|name| WeaponKind::from_symbol_name(&name))
            .filter(|kind| *kind != WeaponKind::Melee)
            .unwrap_or(WeaponKind::Bow);
        for (kind, item, mesh) in [
            (WeaponKind::Melee, melee_item, &mut tracked.melee),
            (ranged_kind, ranged_item, &mut tracked.ranged),
        ] {
            let drawn = match kind {
                WeaponKind::Melee => mode == WeaponMode::Melee,
                _ => mode == WeaponMode::Ranged,
            };
            let target_slot = slots
                .slot_for(kind, drawn)
                .filter(|slot| entities.contains(*slot));
            update_weapon_mesh(
                &mut commands,
                entities,
                mesh,
                item,
                target_slot,
                &gothic_scripts,
                &vfs,
                &mut handles,
                &asset_server,
            );
        }
        if weapon.as_deref() != Some(&tracked) {
            commands.entity(entity).insert(tracked);
        }
    }
}

/// Syncs one weapon mesh child with its equipped item and target slot: creates
/// the mesh child when missing or the item changed, moves it between slots and
/// despawns it when nothing is equipped.
#[allow(clippy::too_many_arguments)]
fn update_weapon_mesh(
    commands: &mut Commands,
    entities: &Entities,
    mesh: &mut WeaponMesh,
    item: Option<InstanceId>,
    target_slot: Option<Entity>,
    gothic_scripts: &Res<ScriptResource>,
    vfs: &Res<ZenginVfs>,
    handles: &mut ResMut<CachedAssetHandles>,
    asset_server: &Res<AssetServer>,
) {
    let path = item.and_then(|item| item_model_path(gothic_scripts, vfs, item));

    let Some(path) = path else {
        // Nothing equipped, or its mesh is unknown
        detach_weapon(commands, entities, mesh);
        return;
    };

    // The weapon mesh child is despawned with the visual when the npc is far
    let child_alive = mesh.entity.is_some_and(|child| entities.contains(child));
    // A different equipped item needs a different mesh child
    let item_changed = mesh.item != item;

    if !child_alive || item_changed {
        // Replace the stale/obsolete weapon mesh child
        if let Some(child) = mesh.entity.filter(|child| entities.contains(*child)) {
            commands.entity(child).despawn();
        }
        let Some(target_slot) = target_slot else {
            // No slot available, only drop the stale child reference
            mesh.entity = None;
            mesh.slot = None;
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
            ))
            .id();
        *mesh = WeaponMesh {
            entity: Some(child),
            item,
            slot: Some(target_slot),
        };
        return;
    }

    mesh.item = item;

    // Move between sheathed and drawn
    if mesh.slot != target_slot
        && let Some(child) = mesh.entity
        && let Some(target_slot) = target_slot
    {
        commands.entity(child).insert(ChildOf(target_slot));
        mesh.slot = Some(target_slot);
    }
}

/// Despawns the weapon mesh child when it is still alive and clears the tracked
/// state.
fn detach_weapon(commands: &mut Commands, entities: &Entities, mesh: &mut WeaponMesh) {
    if let Some(child) = mesh.entity.take()
        && entities.contains(child)
    {
        commands.entity(child).despawn();
    }
    mesh.item = None;
    mesh.slot = None;
}
