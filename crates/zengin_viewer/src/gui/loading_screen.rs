//! Full-screen loading artwork and progress bar shown while a world is built.

use std::sync::PoisonError;

use bevy::asset::LoadState;
use bevy::prelude::*;

use crate::game_state::{AppState, LoadingScreenReady};
use crate::zengin::{LoadingWorld, ZenginVfs, get_full_texture_path, get_zengin_asset_bytes};
use dev_util::perf_debug;
use dev_util::ui_kit::{set_visibility, spawn_gui_root};

/// Loading banner used by worlds without dedicated artwork.
const FALLBACK_BANNER: &str = "LOADING.TGA";

/// Bar rectangle in percent of the window, taken from the original 640x480
/// loading screen.
const BAR_RIGHT: Val = Val::Percent(9.8);
const BAR_TOP: Val = Val::Percent(13.6);
const BAR_WIDTH: Val = Val::Percent(28.0);
const BAR_HEIGHT: Val = Val::Percent(2.7);

/// Frame area inside `PROGRESS.TGA`; the texture is mostly empty margin.
const BAR_FRAME_MIN: Vec2 = Vec2::new(71.0, 20.0);
const BAR_FRAME_MAX: Vec2 = Vec2::new(445.0, 51.0);

/// Fill insets as a percentage of the frame, matching the frame's inner edge.
const FILL_LEFT: f32 = 1.0;
const FILL_TOP: f32 = 17.0;
const FILL_HEIGHT: f32 = 66.0;
/// Fill width at full progress.
const FILL_MAX_WIDTH: f32 = 98.0;

/// Progress gained per second. The bar never reaches full because the loading
/// state ends on its own once the world is ready.
const PROGRESS_RATE: f32 = 0.5;
const PROGRESS_LIMIT: f32 = 0.95;

/// Fallback wait before the world starts even if the loading screen artwork
/// never finishes loading.
const GATE_TIMEOUT: f32 = 5.0;

pub struct LoadingScreenPlugin;

impl Plugin for LoadingScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoadingProgress>();
        app.init_resource::<LoadingScreenGate>();
        app.add_systems(OnEnter(AppState::LoadScreen), open_loading_screen);
        app.add_systems(
            Update,
            (advance_loading_screen_gate, advance_loading_progress)
                .run_if(in_state(AppState::LoadScreen)),
        );
        app.add_systems(OnExit(AppState::LoadScreen), close_loading_screen);
    }
}

#[derive(Component)]
struct LoadingScreenRootMarker;

#[derive(Component)]
struct LoadingBannerMarker;

#[derive(Component)]
struct LoadingBarFrameMarker;

#[derive(Component)]
struct LoadingBarFillMarker;

#[derive(Resource, Default)]
struct LoadingProgress(f32);

/// Whether the loading screen artwork was already visible for one rendered
/// frame. The world preparation waits one more frame so the first heavy loading
/// frame is drawn behind the loading screen.
#[derive(Resource, Default)]
struct LoadingScreenGate {
    assets_shown: bool,
    waited: f32,
}

fn open_loading_screen(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    vfs: Res<ZenginVfs>,
    world: Res<LoadingWorld>,
    mut progress: ResMut<LoadingProgress>,
    mut gate: ResMut<LoadingScreenGate>,
    mut ready: ResMut<LoadingScreenReady>,
    roots: Query<Entity, With<LoadingScreenRootMarker>>,
    mut banners: Query<&mut ImageNode, With<LoadingBannerMarker>>,
    mut fills: Query<&mut Node, With<LoadingBarFillMarker>>,
) {
    let _perf = perf_debug!();
    progress.0 = 0.0;
    gate.assets_shown = false;
    gate.waited = 0.0;
    ready.0 = false;
    let banner = asset_server.load(loading_banner_path(&vfs, &world.0));

    let Some(root) = roots.iter().next() else {
        spawn_loading_screen(&mut commands, &asset_server, banner);
        return;
    };
    commands.entity(root).insert(Visibility::Visible);
    for mut banner_node in &mut banners {
        banner_node.image = banner.clone();
    }
    for mut fill in &mut fills {
        fill.width = Val::Percent(0.0);
    }
}

fn advance_loading_screen_gate(
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut gate: ResMut<LoadingScreenGate>,
    mut ready: ResMut<LoadingScreenReady>,
    banners: Query<&ImageNode, With<LoadingBannerMarker>>,
    frames: Query<&ImageNode, With<LoadingBarFrameMarker>>,
    fills: Query<&ImageNode, With<LoadingBarFillMarker>>,
) {
    let _perf = perf_debug!();
    if ready.0 {
        return;
    }
    if gate.assets_shown {
        ready.0 = true;
        return;
    }
    gate.waited += time.delta_secs();
    if gate.waited >= GATE_TIMEOUT {
        warn!("Loading screen still not ready after {GATE_TIMEOUT}s, starting world load");
        ready.0 = true;
        return;
    }

    let nodes = banners.iter().chain(frames.iter()).chain(fills.iter());
    let mut count = 0usize;
    for node in nodes {
        count += 1;
        match asset_server.get_load_state(node.image.id()) {
            Some(LoadState::Loaded) => {}
            Some(LoadState::Failed(error)) => {
                warn!("Loading screen asset failed, spawning world without it: {error}");
            }
            _ => return,
        }
    }
    if count > 0 {
        gate.assets_shown = true;
    }
}

fn advance_loading_progress(
    time: Res<Time>,
    mut progress: ResMut<LoadingProgress>,
    mut fills: Query<&mut Node, With<LoadingBarFillMarker>>,
) {
    let _perf = perf_debug!();
    progress.0 = (progress.0 + time.delta_secs() * PROGRESS_RATE).min(PROGRESS_LIMIT);
    let width = Val::Percent(FILL_MAX_WIDTH * progress.0);
    for mut fill in &mut fills {
        if fill.width != width {
            fill.width = width;
        }
    }
}

fn close_loading_screen(
    mut roots: Query<&mut Visibility, With<LoadingScreenRootMarker>>,
    mut ready: ResMut<LoadingScreenReady>,
) {
    let _perf = perf_debug!();
    ready.0 = true;
    for mut visibility in &mut roots {
        set_visibility(&mut visibility, false);
    }
}

fn spawn_loading_screen(
    commands: &mut Commands,
    asset_server: &AssetServer,
    banner: Handle<Image>,
) {
    let root = spawn_gui_root(commands, LoadingScreenRootMarker);
    commands.entity(root).insert((
        Visibility::Visible,
        GlobalZIndex(150),
        BackgroundColor(Color::BLACK),
    ));

    let mut banner_node = ImageNode::new(banner);
    banner_node.image_mode = NodeImageMode::Stretch;
    commands.spawn((
        ChildOf(root),
        LoadingBannerMarker,
        banner_node,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
    ));

    spawn_progress_bar(commands, asset_server, root);
}

fn spawn_progress_bar(commands: &mut Commands, asset_server: &AssetServer, parent: Entity) {
    let mut frame = ImageNode::new(asset_server.load(get_full_texture_path("PROGRESS.TGA")));
    frame.image_mode = NodeImageMode::Stretch;
    frame.rect = Some(Rect::new(
        BAR_FRAME_MIN.x,
        BAR_FRAME_MIN.y,
        BAR_FRAME_MAX.x,
        BAR_FRAME_MAX.y,
    ));
    let bar = commands
        .spawn((
            ChildOf(parent),
            LoadingBarFrameMarker,
            frame,
            Node {
                position_type: PositionType::Absolute,
                right: BAR_RIGHT,
                top: BAR_TOP,
                width: BAR_WIDTH,
                height: BAR_HEIGHT,
                ..default()
            },
        ))
        .id();

    let mut fill = ImageNode::new(asset_server.load(get_full_texture_path("PROGRESS_BAR.TGA")));
    fill.image_mode = NodeImageMode::Stretch;
    commands.spawn((
        ChildOf(bar),
        LoadingBarFillMarker,
        fill,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(FILL_LEFT),
            top: Val::Percent(FILL_TOP),
            width: Val::Percent(0.0),
            height: Val::Percent(FILL_HEIGHT),
            ..default()
        },
    ));
}

/// `LOADING_<WORLD>.TGA` for the world basename, or [`FALLBACK_BANNER`] when the
/// world has no dedicated artwork.
fn loading_banner_path(vfs: &ZenginVfs, world_name: &str) -> String {
    let world = world_name.rsplit('/').next().unwrap_or(world_name);
    let candidate = get_full_texture_path(&format!("LOADING_{}.TGA", world.to_uppercase()));
    if asset_exists(vfs, &candidate) {
        return candidate;
    }
    get_full_texture_path(FALLBACK_BANNER)
}

fn asset_exists(vfs: &ZenginVfs, path: &str) -> bool {
    let relative = path.strip_prefix("zengin://").unwrap_or(path);
    let vfs = vfs.0.lock().unwrap_or_else(PoisonError::into_inner);
    get_zengin_asset_bytes(&vfs, relative).is_some()
}
