//! Resource mapping script instance ids to their world entities and back, kept in
//! sync with the ECS via add/change/remove detection on [`GameNpc`] and
//! [`GameItem`].

use std::collections::HashMap;

use bevy::prelude::*;

use crate::zengin::script::symbol_id::InstanceId;
use crate::zengin::{GameItem, GameNpc};

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
/// ECS add/change/remove of [`GameNpc`] and [`GameItem`].
#[derive(Resource, Default)]
pub struct InstanceIndex {
    npcs: EntityIndex,
    items: EntityIndex,
}

impl InstanceIndex {
    /// Entities carrying the given npc instance id.
    pub fn npcs(&self, id: InstanceId) -> &[Entity] {
        self.npcs.entities(id)
    }

    // /// Entities carrying the given item instance id.
    // pub fn items(&self, id: InstanceId) -> &[Entity] {
    //     self.items.entities(id)
    // }

    //     /// Npc instance id of the given entity.
    //     pub fn npc_instance(&self, entity: Entity) -> Option<InstanceId> {
    //         self.npcs.instance(entity)
    //     }
    //
    //     /// Item instance id of the given entity.
    //     pub fn item_instance(&self, entity: Entity) -> Option<InstanceId> {
    //         self.items.instance(entity)
    //     }
}

/// Marks systems that keep [`InstanceIndex`] up to date. Systems that resolve
/// entities through the index should run after this set.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstanceIndexUpdated;

pub struct InstanceIndexPlugin;

impl Plugin for InstanceIndexPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InstanceIndex>();
        app.add_systems(
            Update,
            (track_npcs, track_items).in_set(InstanceIndexUpdated),
        );
    }
}

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
        index.npcs.insert(entity, npc.npc_instance_id);
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
        index.items.insert(entity, item.item_instance_id);
    }
    for entity in removed.read() {
        index.items.remove(entity);
    }
}
