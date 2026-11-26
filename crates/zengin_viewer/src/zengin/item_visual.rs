//! Item runtime data mirrored onto item entities and the mesh update driven by
//! it.

use bevy::prelude::*;

use crate::zengin::ZenginVfs;
use crate::zengin::loaders::vfs::mesh_model_path;
use crate::zengin::resources::{
    CachedAssetHandles, ZenGinModelComponent, ZenGinModelComponentLoaded,
};
use crate::zengin::script::bridge::ScriptChangesApplied;
use crate::zengin::script::state::ItemEntitiesChanged;
use crate::zengin::{GameItem, ItemHandle};
use dev_util::perf_debug;

pub struct ItemVisualPlugin;

impl Plugin for ItemVisualPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (seed_item_data_on_add, sync_item_data, apply_item_visual)
                .chain()
                .after(ScriptChangesApplied),
        );
    }
}

/// Runtime data of an item instance mirrored from the script handle.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct ItemData {
    /// Instance (symbol) name.
    pub name: String,
    /// Visual set on the item instance.
    pub visual: String,
    /// Visual override (`visual_change`), takes precedence when set.
    pub visual_change: Option<String>,
}

impl ItemData {
    /// Visual used for rendering, the override when set.
    pub fn chosen_visual(&self) -> &str {
        self.visual_change.as_deref().unwrap_or(&self.visual)
    }

    fn from_handle(handle: &ItemHandle) -> Self {
        let runtime = handle
            .runtime()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Self {
            name: runtime.name.clone(),
            visual: runtime.visual.clone(),
            visual_change: runtime.visual_change.clone(),
        }
    }
}

/// Mirrors the handle data onto newly spawned item entities.
fn seed_item_data_on_add(
    mut commands: Commands,
    items: Query<(Entity, &GameItem), Added<GameItem>>,
) {
    let _perf = perf_debug!();
    for (entity, item) in &items {
        commands
            .entity(entity)
            .try_insert(ItemData::from_handle(item.handle()));
    }
}

/// Mirrors item changes onto the entities carrying the instance.
fn sync_item_data(
    mut commands: Commands,
    mut changes: MessageReader<ItemEntitiesChanged>,
    items: Query<&ItemData>,
) {
    let _perf = perf_debug!();
    for change in changes.read() {
        let data = ItemData::from_handle(&change.handle);
        for &entity in &change.entities {
            if items.get(entity).ok() == Some(&data) {
                continue;
            }
            commands.entity(entity).try_insert(data.clone());
        }
    }
}

/// Rebuilds the rendered item mesh when its visual changed.
fn apply_item_visual(
    mut commands: Commands,
    vfs: Res<ZenginVfs>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut items: Query<(Entity, &ItemData, &mut ZenGinModelComponent), Changed<ItemData>>,
) {
    let _perf = perf_debug!();
    for (entity, data, mut model) in &mut items {
        let Some(path) = mesh_model_path(&vfs, data.chosen_visual()) else {
            warn!(
                "No mesh path for item({}) visual({})",
                data.name,
                data.chosen_visual()
            );
            continue;
        };
        let handle = handles.get_model_handle(&asset_server, &path, None);
        if model.model_handle == handle {
            continue;
        }
        model.model_handle = handle;
        commands
            .entity(entity)
            .remove::<ZenGinModelComponentLoaded>();
        commands.entity(entity).despawn_children();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zengin::ItemHandle;
    use crate::zengin::script::state::ItemRuntime;
    use crate::zengin::script::symbol_id::InstanceId;
    use daedalus_typed::Item;
    use std::sync::{Arc, RwLock};

    #[test]
    fn sync_item_data_mirrors_the_runtime_visual() {
        let mut app = App::new();
        app.add_message::<ItemEntitiesChanged>();
        app.add_systems(Update, sync_item_data);

        let entity = app.world_mut().spawn_empty().id();
        let handle = ItemHandle::new(
            InstanceId::new(42).expect("valid instance id"),
            Arc::new(RwLock::new(Item::default())),
            Arc::new(RwLock::new(ItemRuntime {
                name: "ITMI_TEST".to_string(),
                visual: "VISUAL".to_string(),
                visual_change: Some("VISUAL_CHANGE".to_string()),
            })),
        );
        app.world_mut()
            .resource_mut::<Messages<ItemEntitiesChanged>>()
            .write(ItemEntitiesChanged {
                handle,
                entities: vec![entity],
            });

        app.update();

        let data = app.world().get::<ItemData>(entity).expect("item data");
        assert_eq!(data.name, "ITMI_TEST");
        assert_eq!(data.chosen_visual(), "VISUAL_CHANGE");
    }
}
