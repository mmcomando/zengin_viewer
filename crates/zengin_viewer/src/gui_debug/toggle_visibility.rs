use bevy::prelude::*;

use crate::zengin::{GameChest, GameItem, GameMob, GameNpc, StaticMesh, WorldMesh};
use dev_util::perf_debug;

#[derive(Default)]
pub struct ToggleVisibility;

impl Plugin for ToggleVisibility {
    fn build(&self, app: &mut App) {
        app.add_message::<OptionsChanged>();
        app.add_systems(Startup, insert_resources);

        app.add_systems(Update, on_show_options_changed);
    }
}

fn insert_resources(mut commands: Commands) {
    let _perf = perf_debug!();
    commands.insert_resource(ShowOptions::default());
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Resource)]
pub struct ShowOptions {
    pub show_world_mesh: bool,
    pub show_static_meshes: bool,
    pub show_npcs: bool,
    pub show_chests: bool,
    pub show_mobs: bool,
    pub show_items: bool,
}

impl Default for ShowOptions {
    fn default() -> Self {
        Self {
            show_world_mesh: true,
            show_static_meshes: true,
            show_npcs: true,
            show_chests: true,
            show_mobs: true,
            show_items: true,
        }
    }
}

#[derive(Message, Default)]
pub struct OptionsChanged;

/// Query data used to toggle visibility of world entities
pub type VisibilityQueryData<'a> = (
    &'a mut Visibility,
    Option<&'a WorldMesh>,
    Option<&'a StaticMesh>,
    Option<&'a GameNpc>,
    Option<&'a GameChest>,
    Option<&'a GameMob>,
    Option<&'a GameItem>,
);

fn on_show_options_changed(
    messages: MessageReader<OptionsChanged>,
    options: ResMut<ShowOptions>,
    mut query: Query<VisibilityQueryData<'_>>,
) {
    let _perf = perf_debug!();
    if messages.is_empty() {
        return;
    }

    let world_mesh_vis = if options.show_world_mesh {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let static_mesh_vis = if options.show_static_meshes {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let npc_vis = if options.show_npcs {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let chest_vis = if options.show_chests {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let mob_vis = if options.show_mobs {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    let item_vis = if options.show_items {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    for (mut visibility, world_mesh, static_mesh, npc, chest, mob, item) in &mut query {
        if world_mesh.is_some() {
            *visibility = world_mesh_vis;
        }
        if static_mesh.is_some() {
            *visibility = static_mesh_vis;
        }
        if npc.is_some() {
            *visibility = npc_vis;
        }
        if chest.is_some() {
            *visibility = chest_vis;
        }
        if mob.is_some() {
            *visibility = mob_vis;
        }
        if item.is_some() {
            *visibility = item_vis;
        }
    }
}
