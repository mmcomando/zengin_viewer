use bevy::{
    camera::primitives::Aabb,
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};

/// Automatically assign objects Distance components based on distance from camera.
/// It allows other systems to iterate only on interesting entities.
/// It allows to efficiently reduce quality of objects based on distance.
#[derive(Default)]
pub struct DistancePlugin;

impl Plugin for DistancePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MainCameraPos>();

        app.add_systems(Update, get_camera_pos);
        app.add_systems(Update, update_distance_state_far);
        app.add_systems(Update, update_distance_state_near);
        app.add_systems(Update, make_visible);
        app.add_systems(Update, make_hidden);
    }
}

#[derive(Resource, Default)]
pub struct MainCameraPos {
    tr: Transform,
}

#[derive(Component, Default)]
#[component(on_add = on_classify_added)]
pub struct ClassifyDistance {}

/// Object is far away.
/// Object might consider:
/// - be made invisible
/// - reduce quality
/// - reduce simulation quality
#[derive(Component, Default)]
pub struct DistanceFar {}

#[derive(Component, Default)]
pub struct DistanceNear {}

#[derive(Component, Default)]
pub struct EntityDistanceState {}

const DIST_MAKE_FAR: f32 = 200.0;
const DIST_MAKE_FAR_SQUARED: f32 = DIST_MAKE_FAR * DIST_MAKE_FAR;

/// Create hysteresis between transitions from far to near
/// It is needed for two reasons:
/// - Make objects popping less visible
/// - Prevent constant loading/unloading of objects at the edge of visibility
const FAR_OVERSHOOT: f32 = 1.1;

fn on_classify_added(mut world: DeferredWorld, context: HookContext) {
    world
        .commands()
        .entity(context.entity)
        .insert((EntityDistanceState::default(), DistanceFar::default()));
}

fn get_camera_pos(mut state: ResMut<MainCameraPos>, query: Query<&Transform, With<Camera3d>>) {
    let mut transforms = query.iter();
    if let Some(tr) = transforms.next() {
        state.tr = *tr;
        if transforms.next().is_some() {
            error!("More than one Camera3d found, distance classification uses an arbitrary one");
        }
    }
}

// Larger objects are hidden later
fn obj_is_visible(camera_tr: &Transform, gl_tr: &GlobalTransform, overshoot: f32) -> bool {
    let pos_dt = camera_tr.translation - gl_tr.translation();
    let distance_squared = pos_dt.length_squared();
    return distance_squared < DIST_MAKE_FAR_SQUARED * overshoot;
}

fn update_distance_state_far(
    mut commands: Commands,
    state: Res<MainCameraPos>,
    query: Query<(Entity, &GlobalTransform, Option<&Aabb>), With<DistanceFar>>,
) {
    for (entity_id, tr, aabb) in query {
        let obj_size = aabb.map_or(1.0, |el| el.max().length());

        if obj_is_visible(&state.tr, tr, obj_size) {
            let mut entity_cmd = commands.entity(entity_id);
            entity_cmd.remove::<DistanceFar>();
            entity_cmd.insert(DistanceNear::default());
        }
    }
}

fn update_distance_state_near(
    mut commands: Commands,
    state: Res<MainCameraPos>,
    query: Query<(Entity, &GlobalTransform, Option<&Aabb>), With<DistanceNear>>,
) {
    for (entity_id, tr, aabb) in query {
        let obj_size = aabb.map_or(1.0, |el| el.max().length());
        if !obj_is_visible(&state.tr, tr, obj_size * FAR_OVERSHOOT) {
            let mut entity_cmd = commands.entity(entity_id);
            entity_cmd.remove::<DistanceNear>();
            entity_cmd.insert(DistanceFar::default());
        }
    }
}

fn make_visible(query: Query<&mut Visibility, Added<DistanceNear>>) {
    for mut visibility in query {
        *visibility = Visibility::Inherited;
    }
}

fn make_hidden(query: Query<&mut Visibility, Added<DistanceFar>>) {
    for mut visibility in query {
        *visibility = Visibility::Hidden;
    }
}
