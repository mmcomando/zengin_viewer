use avian3d::parry::shape::TypedShape;
use avian3d::prelude::*;
use bevy::{color::palettes::tailwind, prelude::*};

use crate::graphics::animation::AnimatedJoint;
use crate::graphics::disco_lamp::DrawGizmo;
use crate::zengin::WaypointGraph;

/// Which gizmos are currently rendered, toggled from the debug UI.
#[derive(Resource, Default)]
pub struct GizmosOptions {
    pub convex_colliders: bool,
    pub trimesh_colliders: bool,
    pub characters_colliders: bool,
    pub bones: bool,
    pub lights: bool,
    pub way_points: bool,
}

pub struct GizmosPlugin;

impl Plugin for GizmosPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GizmosOptions>();

        app.add_systems(
            Update,
            draw_convex_hull_gizmos.run_if(|options: Res<GizmosOptions>| options.convex_colliders),
        );
        app.add_systems(
            Update,
            draw_bones.run_if(|options: Res<GizmosOptions>| options.bones),
        );
        app.add_systems(
            Update,
            draw_trimesh_gizmos.run_if(|options: Res<GizmosOptions>| options.trimesh_colliders),
        );
        app.add_systems(
            Update,
            draw_characters_colliders
                .run_if(|options: Res<GizmosOptions>| options.characters_colliders),
        );
        app.add_systems(
            Update,
            draw_lights.run_if(|options: Res<GizmosOptions>| options.lights),
        );
        app.add_systems(
            Update,
            draw_waypoints.run_if(|options: Res<GizmosOptions>| options.way_points),
        );
    }
}

fn draw_convex_hull_gizmos(mut gizmos: Gizmos, colliders: Query<(&Collider, &GlobalTransform)>) {
    for (collider, transform) in &colliders {
        let TypedShape::ConvexPolyhedron(hull) = collider.shape().as_typed_shape() else {
            continue;
        };
        let isometry = Isometry3d::new(transform.translation(), transform.rotation());
        let points = hull.points();
        let vertices_adj_to_face = hull.vertices_adj_to_face();
        for face in hull.faces() {
            let first = face.first_vertex_or_edge as usize;
            let count = face.num_vertices_or_edges as usize;
            if count < 3 {
                continue;
            }
            for i in 0..count {
                let a = points[vertices_adj_to_face[first + i] as usize];
                let b = points[vertices_adj_to_face[first + (i + 1) % count] as usize];
                gizmos.line(isometry * a, isometry * b, tailwind::GREEN_400);
            }
        }
    }
}

fn draw_trimesh_gizmos(mut gizmos: Gizmos, colliders: Query<(&Collider, &GlobalTransform)>) {
    for (collider, transform) in &colliders {
        let TypedShape::TriMesh(trimesh) = collider.shape().as_typed_shape() else {
            continue;
        };
        let isometry = Isometry3d::new(transform.translation(), transform.rotation());
        for triangle in trimesh.triangles() {
            let vertices = [triangle.a, triangle.b, triangle.c];
            for i in 0..3 {
                gizmos.line(
                    isometry * vertices[i],
                    isometry * vertices[(i + 1) % 3],
                    tailwind::CYAN_400,
                );
            }
        }
    }
}

/// Draws a wireframe around characters collider
fn draw_characters_colliders(mut gizmos: Gizmos, entities: Query<(&Collider, &GlobalTransform)>) {
    for (collider, transform) in &entities {
        let Some(capsule) = collider.shape().as_capsule() else {
            continue;
        };
        let isometry = Isometry3d::new(transform.translation(), transform.rotation());
        gizmos.primitive_3d(
            &Capsule3d::new(capsule.radius, capsule.half_height()),
            isometry,
            tailwind::RED_500,
        );
    }
}

fn draw_bones(
    mut gizmos: Gizmos,

    bones: Query<(Entity, &GlobalTransform, Option<&ChildOf>), With<AnimatedJoint>>,
    transforms: Query<&GlobalTransform>,
) {
    for (_bone_entity, bone_transform, parent) in bones.iter() {
        if let Some(parent) = parent
            && let Ok(parent_transform) = transforms.get(parent.0)
        {
            // Draw a line from parent bone to current bone
            gizmos.line(
                parent_transform.translation(),
                bone_transform.translation(),
                Color::srgb(0.0, 1.0, 0.0), // Green bones
            );

            // Optional: Draw the bone itself (a small sphere)
            // gizmos.sphere(
            //     bone_transform.translation(),
            //     // Quat::IDENTITY,
            //     0.02,
            //     Color::srgb(1.0, 0.0, 0.0), // Red joints
            // );
        }
    }
}

fn draw_lights(mut gizmos: Gizmos, query: Query<(&GlobalTransform, &DrawGizmo)>) {
    for (tr, draw_gizmo) in query {
        gizmos.sphere(tr.translation(), 0.1, draw_gizmo.color);
    }
}

fn draw_waypoints(mut gizmos: Gizmos, graph: Option<Res<WaypointGraph>>) {
    let Some(graph) = graph else {
        return;
    };
    for node in &graph.nodes {
        let color = if node.free {
            Color::srgb(1.0, 0.6, 0.0)
        } else {
            Color::srgb(0.2, 0.7, 1.0)
        };
        gizmos.sphere(node.position, 0.3, color);
    }
    let edge_color = Color::srgb(0.4, 0.4, 0.4);
    for (index, neighbors) in graph.adjacency.iter().enumerate() {
        for &(neighbor, _) in neighbors {
            // Draw each undirected edge once
            if neighbor <= index {
                continue;
            }
            gizmos.line(
                graph.nodes[index].position,
                graph.nodes[neighbor].position,
                edge_color,
            );
        }
    }
}
