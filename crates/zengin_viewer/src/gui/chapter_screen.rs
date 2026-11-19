//! Chapter splash shown by `IntroduceChapter`: a centered 800x600 background
//! texture with the chapter title near its top and the subtitle near its
//! bottom. The overlay blocks gameplay input and pauses the game clock, so the
//! world does not advance behind it. The auto-close timer runs on real time.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::audio::PlaySound;
use crate::ui_kit::{UiText, set_visibility, spawn_gui_root};
use crate::zengin::{
    ChapterIntro, ScriptChangesApplied, ZenginVfs, resolve_sound_asset, resolve_texture_asset,
};

/// Original chapter image size in interface units.
const CHAPTER_IMAGE_SIZE: Vec2 = Vec2::new(800.0, 600.0);
/// Distance of the title and subtitle from the image edges.
const EDGE_OFFSET: f32 = 50.0;
const TITLE_FONT_SIZE: f32 = 24.0;
const SUBTITLE_FONT_SIZE: f32 = 18.0;
/// Above the dialog/menu overlays and the loading screen.
const CHAPTER_Z_INDEX: i32 = 200;

pub(crate) struct ChapterScreenPlugin;

impl Plugin for ChapterScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChapterScreenState>();
        app.add_systems(Startup, spawn_chapter_root);
        app.configure_sets(Update, ChapterScreenUpdated.after(ScriptChangesApplied));
        app.add_systems(
            Update,
            (
                show_chapter_screen.in_set(ChapterScreenUpdated),
                tick_chapter_screen.run_if(is_chapter_open),
                sync_chapter_visibility.run_if(resource_changed::<ChapterScreenState>),
                // Applied after the transitions above, so a splash opened or
                // auto-closed this frame is paused or resumed the same frame.
                sync_time_pause
                    .after(show_chapter_screen)
                    .after(tick_chapter_screen),
            ),
        );
    }
}

/// Marks the system that applies a newly requested chapter splash, so input
/// mode switching runs after the screen is open.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ChapterScreenUpdated;

/// Whether the chapter splash is currently displayed and how long it has left.
#[derive(Resource, Default)]
pub(crate) struct ChapterScreenState {
    open: bool,
    time_left_secs: Option<f32>,
}

impl ChapterScreenState {
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    /// Hides the splash; the visibility sync system applies it.
    pub(crate) fn close(&mut self) {
        *self = Self::default();
    }
}

/// Run condition for the timer system.
fn is_chapter_open(state: Res<ChapterScreenState>) -> bool {
    state.open
}

#[derive(Component)]
struct ChapterRootMarker;

fn spawn_chapter_root(mut commands: Commands) {
    let root = spawn_gui_root(&mut commands, ChapterRootMarker);
    commands.entity(root).insert((
        GlobalZIndex(CHAPTER_Z_INDEX),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
    ));
}

#[allow(clippy::too_many_arguments)]
fn show_chapter_screen(
    mut commands: Commands,
    mut intros: MessageReader<ChapterIntro>,
    vfs: Res<ZenginVfs>,
    asset_server: Res<AssetServer>,
    mut sounds: MessageWriter<PlaySound>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut state: ResMut<ChapterScreenState>,
    root: Single<(Entity, &mut Visibility), With<ChapterRootMarker>>,
) {
    // Only the newest request of the frame matters, a later `IntroduceChapter`
    // replaces an earlier one.
    let Some(intro) = intros.read().last() else {
        return;
    };
    let Some(texture) = resolve_texture_asset(&vfs, &intro.texture) else {
        warn!(
            "Chapter intro({}) skipped: texture({}) not found",
            intro.title, intro.texture
        );
        return;
    };
    play_chapter_sound(intro, &vfs, &mut sounds);

    let (root_entity, mut visibility) = root.into_inner();
    set_visibility(&mut visibility, true);
    commands.entity(root_entity).despawn_children();

    let scale = windows.iter().next().map_or(1.0, Window::scale_factor);
    spawn_chapter_image(
        &mut commands,
        &asset_server,
        root_entity,
        texture,
        &intro.title,
        &intro.subtitle,
        scale,
    );
    state.open = true;
    state.time_left_secs = (intro.time_ms > 0).then(|| intro.time_ms as f32 / 1000.0);
}

fn play_chapter_sound(
    intro: &ChapterIntro,
    vfs: &ZenginVfs,
    sounds: &mut MessageWriter<PlaySound>,
) {
    if intro.sound.is_empty() {
        return;
    }
    let Some(path) = resolve_sound_asset(vfs, &intro.sound) else {
        warn!(
            "Chapter intro({}) sound({}) not found",
            intro.title, intro.sound
        );
        return;
    };
    sounds.write(PlaySound {
        asset_path: path,
        position: None,
    });
}

fn spawn_chapter_image(
    commands: &mut Commands,
    asset_server: &AssetServer,
    root: Entity,
    texture: String,
    title: &str,
    subtitle: &str,
    scale: f32,
) {
    let container = commands
        .spawn((
            ChildOf(root),
            Node {
                width: Val::Px(CHAPTER_IMAGE_SIZE.x * scale),
                height: Val::Px(CHAPTER_IMAGE_SIZE.y * scale),
                position_type: PositionType::Absolute,
                ..default()
            },
        ))
        .id();

    let mut image = ImageNode::new(asset_server.load(texture));
    image.image_mode = NodeImageMode::Stretch;
    commands.spawn((
        ChildOf(container),
        image,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
    ));

    commands.spawn((
        ChildOf(container),
        chapter_text(title, TITLE_FONT_SIZE, scale),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(EDGE_OFFSET * scale),
            width: Val::Percent(100.0),
            ..default()
        },
    ));
    commands.spawn((
        ChildOf(container),
        chapter_text(subtitle, SUBTITLE_FONT_SIZE, scale),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(EDGE_OFFSET * scale),
            width: Val::Percent(100.0),
            ..default()
        },
    ));
}

fn chapter_text(text: &str, font_size: f32, scale: f32) -> UiText {
    UiText::new(text, font_size * scale, Color::WHITE).justify(Justify::Center)
}

fn tick_chapter_screen(
    time: Res<Time<Real>>,
    mut state: ResMut<ChapterScreenState>,
    mut root: Single<&mut Visibility, With<ChapterRootMarker>>,
) {
    let Some(left) = state.time_left_secs else {
        return;
    };
    let left = left - time.delta_secs();
    if left > 0.0 {
        state.time_left_secs = Some(left);
        return;
    }
    state.close();
    set_visibility(&mut root, false);
}

/// Pauses the game clock while the splash is visible. Pausing [`Time<Virtual>`]
/// also freezes fixed-step physics, so the whole world stands still; the splash
/// timer is unaffected because it uses [`Time<Real>`].
fn sync_time_pause(
    state: Res<ChapterScreenState>,
    mut was_open: Local<bool>,
    mut time: ResMut<Time<Virtual>>,
) {
    if state.open == *was_open {
        return;
    }
    *was_open = state.open;
    if state.open {
        time.pause();
    } else {
        time.unpause();
    }
}

/// Applies closes performed outside the timer system (the Back key).
fn sync_chapter_visibility(
    state: Res<ChapterScreenState>,
    mut root: Single<&mut Visibility, With<ChapterRootMarker>>,
) {
    set_visibility(&mut root, state.open);
}
