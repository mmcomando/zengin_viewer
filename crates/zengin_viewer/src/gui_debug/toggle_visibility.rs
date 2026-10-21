use bevy::prelude::*;

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
    commands.insert_resource(ShowOptions::default());
}

#[derive(Component, Default)]
pub struct NpcVisibility {}

#[derive(Component, Default)]
pub struct WorldMesh {}

#[derive(Component, Default)]
pub struct StaticMesh {}

#[allow(clippy::struct_excessive_bools)]
#[derive(Resource)]
pub struct ShowOptions {
    pub show_world_mesh: bool,
    pub show_static_meshes: bool,
    pub show_npcs: bool,
}

impl Default for ShowOptions {
    fn default() -> Self {
        Self {
            show_world_mesh: true,
            show_static_meshes: true,
            show_npcs: true,
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
    Option<&'a NpcVisibility>,
);

fn on_show_options_changed(
    messages: MessageReader<OptionsChanged>,
    options: ResMut<ShowOptions>,
    mut query: Query<VisibilityQueryData<'_>>,
) {
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

    for (mut visibility, world_mesh, static_mesh, npc) in &mut query {
        if world_mesh.is_some() {
            *visibility = world_mesh_vis;
        }
        if static_mesh.is_some() {
            *visibility = static_mesh_vis;
        }
        if npc.is_some() {
            *visibility = npc_vis;
        }
    }
}
