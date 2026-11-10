//! Resource mapping script instance ids to their world entities and back, kept in
//! sync with the ECS via add/change/remove detection on [`GameNpc`] and
//! [`GameItem`].

use std::collections::HashMap;

use bevy::prelude::*;

use crate::game_state::AppState;
use crate::zengin::script::symbol_id::InstanceId;
use crate::zengin::{GameItem, GameNpc, ItemHandle, NpcHandle};

pub struct InstanceIndexPlugin;

impl Plugin for InstanceIndexPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InstanceIndex>();
        app.add_systems(
            Update,
            (track_npcs, track_items)
                .in_set(InstanceIndexUpdated)
                .run_if(in_state(AppState::InGame)),
        );
    }
}

/// Two-way mapping between instance ids and the entities carrying them. One
/// instance id can be carried by several entities (e.g. duplicated items), so
/// the forward side holds a list. Only the small per-instance list is scanned on
/// remove, the maps are never scanned.
#[derive(Default)]
struct EntityIndex {
    /// Instance id -> entities carrying it.
    by_instance: HashMap<InstanceId, Vec<Entity>>,
    /// Entity -> its instance id.
    by_entity: HashMap<Entity, InstanceId>,
}

impl EntityIndex {
    fn insert(&mut self, entity: Entity, id: InstanceId) {
        self.by_instance.entry(id).or_default().push(entity);
        self.by_entity.insert(entity, id);
    }

    fn remove(&mut self, entity: Entity) {
        let Some(id) = self.by_entity.remove(&entity) else {
            return;
        };
        let Some(entities) = self.by_instance.get_mut(&id) else {
            return;
        };
        // Instances hold at most a few entities, scan the small list
        entities.retain(|tracked| *tracked != entity);

        if entities.is_empty() {
            self.by_instance.remove(&id);
        }
    }

    fn entities(&self, id: InstanceId) -> &[Entity] {
        self.by_instance.get(&id).map_or(&[], Vec::as_slice)
    }

    // fn instance(&self, entity: Entity) -> Option<InstanceId> {
    //     self.by_entity.get(&entity).copied()
    // }
}

/// Two-way index of every npc and item instance entity, kept in sync from the
/// ECS add/change/remove of [`GameNpc`] and [`GameItem`]. It also owns the shared
/// npc handles, so an instance id resolves to both its data and its entities
/// with one lookup.
#[derive(Resource, Default)]
pub struct InstanceIndex {
    npcs: EntityIndex,
    items: EntityIndex,
    npc_handles: HashMap<InstanceId, NpcHandle>,
    item_handles: HashMap<InstanceId, ItemHandle>,
}

impl InstanceIndex {
    /// Drops every tracked entity, instance and handle, used when the world is
    /// restarted and all of them are rebuilt.
    pub fn clear(&mut self) {
        self.npcs = EntityIndex::default();
        self.items = EntityIndex::default();
        self.npc_handles.clear();
        self.item_handles.clear();
    }

    /// Entities carrying the given npc instance id.
    pub fn npcs(&self, id: InstanceId) -> &[Entity] {
        self.npcs.entities(id)
    }

    /// Entities carrying the given item instance id.
    pub fn items(&self, id: InstanceId) -> &[Entity] {
        self.items.entities(id)
    }

    /// Shared handle of the given npc instance, `None` before the script has
    /// created its handle.
    pub fn npc_handle(&self, id: InstanceId) -> Option<&NpcHandle> {
        self.npc_handles.get(&id)
    }

    /// All shared npc handles, including instances without a spawned entity.
    #[allow(dead_code)] // kept for the upcoming dead-state computation
    pub fn npc_handles(&self) -> impl Iterator<Item = &NpcHandle> {
        self.npc_handles.values()
    }

    /// Registers a shared npc handle, replacing an older handle of the same
    /// instance.
    pub fn insert_npc_handle(&mut self, handle: NpcHandle) {
        self.npc_handles.insert(handle.instance_id(), handle);
    }

    /// Registers the handles created by a script load, replacing any handles
    /// left over from a previous script instance.
    pub fn replace_npc_handles(&mut self, handles: impl IntoIterator<Item = NpcHandle>) {
        self.npc_handles.clear();
        self.insert_npc_handles(handles);
    }

    /// Registers the handles created by a script load.
    pub fn insert_npc_handles(&mut self, handles: impl IntoIterator<Item = NpcHandle>) {
        for handle in handles {
            self.insert_npc_handle(handle);
        }
    }

    /// Shared handle of the given item instance, `None` before the script has
    /// created its handle.
    pub fn item_handle(&self, id: InstanceId) -> Option<&ItemHandle> {
        self.item_handles.get(&id)
    }

    /// Registers a shared item handle, replacing an older handle of the same
    /// instance.
    pub fn insert_item_handle(&mut self, handle: ItemHandle) {
        self.item_handles.insert(handle.instance_id(), handle);
    }

    /// Registers the item handles created by a script load, replacing any
    /// handles left over from a previous script instance.
    pub fn replace_item_handles(&mut self, handles: impl IntoIterator<Item = ItemHandle>) {
        self.item_handles.clear();
        for handle in handles {
            self.insert_item_handle(handle);
        }
    }
}

/// Marks systems that keep [`InstanceIndex`] up to date. Systems that resolve
/// entities through the index should run after this set.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstanceIndexUpdated;

/// Inserts, updates and removes npc instance mappings. `Changed` also reports
/// newly added components, so it covers both add and change.
fn track_npcs(
    mut index: ResMut<InstanceIndex>,
    npcs: Query<(Entity, &GameNpc), Changed<GameNpc>>,
    mut removed: RemovedComponents<GameNpc>,
) {
    for (entity, npc) in &npcs {
        // Drop any previous mapping first, the instance id may have changed
        index.npcs.remove(entity);
        index.npcs.insert(entity, npc.instance_id());
    }
    for entity in removed.read() {
        index.npcs.remove(entity);
    }
}

/// Inserts, updates and removes item instance mappings. `Changed` also reports
/// newly added components, so it covers both add and change.
fn track_items(
    mut index: ResMut<InstanceIndex>,
    items: Query<(Entity, &GameItem), Changed<GameItem>>,
    mut removed: RemovedComponents<GameItem>,
) {
    for (entity, item) in &items {
        // Drop any previous mapping first, the instance id may have changed
        index.items.remove(entity);
        index.items.insert(entity, item.instance_id());
    }
    for entity in removed.read() {
        index.items.remove(entity);
    }
}
