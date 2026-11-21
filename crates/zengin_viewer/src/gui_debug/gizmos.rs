use avian3d::parry::shape::TypedShape;
use avian3d::prelude::*;
use bevy::{color::palettes::tailwind, prelude::*};

use crate::graphics::AnimatedJoint;
use crate::zengin::WaypointGraph;

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
            draw_cuboid_gizmos.run_if(|options: Res<GizmosOptions>| options.cuboid_colliders),
        );
        app.add_systems(
            Update,
            draw_sphere_gizmos.run_if(|options: Res<GizmosOptions>| options.sphere_colliders),
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
            draw_waypoints.run_if(|options: Res<GizmosOptions>| options.way_points),
        );
    }
}

/// Which gizmos are currently rendered, toggled from the debug UI.
#[derive(Resource, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct GizmosOptions {
    pub convex_colliders: bool,
    pub cuboid_colliders: bool,
    pub sphere_colliders: bool,
    pub trimesh_colliders: bool,
    pub characters_colliders: bool,
    pub bones: bool,
    pub way_points: bool,
}

/// Maximum distance in meters from the camera at which gizmos are drawn. It
/// keeps the large world datasets from flooding the overlay.
const GIZMO_MAX_DISTANCE: f32 = 30.0;

fn within_gizmo_distance(camera_position: Vec3, point: Vec3) -> bool {
    camera_position.distance_squared(point) <= GIZMO_MAX_DISTANCE * GIZMO_MAX_DISTANCE
}

fn draw_convex_hull_gizmos(
    mut gizmos: Gizmos,
    colliders: Query<(&Collider, &GlobalTransform)>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
) {
    let camera_position = camera.translation();
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
                let aa = isometry * points[vertices_adj_to_face[first + i] as usize];
                let bb = isometry * points[vertices_adj_to_face[first + (i + 1) % count] as usize];
                if !within_gizmo_distance(camera_position, (aa + bb) / 2.0) {
                    continue;
                }
                gizmos.line(aa, bb, tailwind::GREEN_400);
            }
        }
    }
}

fn draw_cuboid_gizmos(
    mut gizmos: Gizmos,
    colliders: Query<(&Collider, &GlobalTransform)>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
) {
    let camera_position = camera.translation();
    for (collider, transform) in &colliders {
        let TypedShape::Cuboid(cuboid) = collider.shape().as_typed_shape() else {
            continue;
        };
        if !within_gizmo_distance(camera_position, transform.translation()) {
            continue;
        }
        let isometry = Isometry3d::new(transform.translation(), transform.rotation());
        let half = cuboid.half_extents;
        gizmos.primitive_3d(
            &bevy::shape::Cuboid::new(half.x * 2.0, half.y * 2.0, half.z * 2.0),
            isometry,
            tailwind::BLUE_400,
        );
    }
}

fn draw_sphere_gizmos(
    mut gizmos: Gizmos,
    colliders: Query<(&Collider, &GlobalTransform)>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
) {
    let camera_position = camera.translation();
    for (collider, transform) in &colliders {
        let TypedShape::Ball(ball) = collider.shape().as_typed_shape() else {
            continue;
        };
        if !within_gizmo_distance(camera_position, transform.translation()) {
            continue;
        }
        gizmos.sphere(transform.translation(), ball.radius, tailwind::AMBER_400);
    }
}

fn draw_trimesh_gizmos(
    mut gizmos: Gizmos,
    colliders: Query<(&Collider, &GlobalTransform)>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
) {
    let camera_position = camera.translation();
    for (collider, transform) in &colliders {
        let TypedShape::TriMesh(trimesh) = collider.shape().as_typed_shape() else {
            continue;
        };
        let isometry = Isometry3d::new(transform.translation(), transform.rotation());
        for triangle in trimesh.triangles() {
            let vertices = [
                isometry * triangle.a,
                isometry * triangle.b,
                isometry * triangle.c,
            ];
            let center = (vertices[0] + vertices[1] + vertices[2]) / 3.0;
            if !within_gizmo_distance(camera_position, center) {
                continue;
            }
            for i in 0..3 {
                gizmos.line(vertices[i], vertices[(i + 1) % 3], tailwind::CYAN_400);
            }
        }
    }
}

/// Draws a wireframe around characters collider
fn draw_characters_colliders(
    mut gizmos: Gizmos,
    entities: Query<(&Collider, &GlobalTransform)>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
) {
    let camera_position = camera.translation();
    for (collider, transform) in &entities {
        let Some(capsule) = collider.shape().as_capsule() else {
            continue;
        };
        if !within_gizmo_distance(camera_position, transform.translation()) {
            continue;
        }
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
    camera: Single<&GlobalTransform, With<Camera3d>>,
) {
    let camera_position = camera.translation();
    for (_bone_entity, bone_transform, parent) in bones.iter() {
        let Some(parent) = parent else {
            continue;
        };
        let Ok(parent_transform) = transforms.get(parent.0) else {
            continue;
        };
        let parent_position = parent_transform.translation();
        let bone_position = bone_transform.translation();
        if !within_gizmo_distance(camera_position, (parent_position + bone_position) / 2.0) {
            continue;
        }
        // Draw a line from parent bone to current bone
        gizmos.line(
            parent_position,
            bone_position,
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

fn draw_waypoints(
    mut gizmos: Gizmos,
    graph: Option<Res<WaypointGraph>>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
) {
    let Some(graph) = graph else {
        return;
    };
    let camera_position = camera.translation();
    for node in &graph.nodes {
        if !within_gizmo_distance(camera_position, node.position) {
            continue;
        }
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
            let start = graph.nodes[index].position;
            let end = graph.nodes[neighbor].position;
            if !within_gizmo_distance(camera_position, (start + end) / 2.0) {
                continue;
            }
            gizmos.line(start, end, edge_color);
        }
    }
}
