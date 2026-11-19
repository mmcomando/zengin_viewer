use std::collections::HashSet;

use bevy::prelude::*;

use crate::gameplay::objects::{NpcBodyRootOffset, NpcModel};
use crate::graphics::{
    AnimationController, AutoAnimSequencer, DesiredAnimation, SetDesiredAnimation,
};
use crate::zengin::common::{
    MAX_AI_USE_DISTANCE, MOB_SEARCH_RADIUS, face_direction_quat, horizontal_offset,
};
use crate::zengin::npc_actions::action::ExecutingActionMarker;
use crate::zengin::npc_routines::{
    AnimationFiles, NPC_BODY_Y_OFFSET, NpcOverlay, has_animation, queue_named_animation,
    queue_root_motion_animation, queue_stand_animation,
};
use crate::zengin::resources::{CachedAssetHandles, ZenGinModelComponent};
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::{GameMob, GameNpc, MobOccupant, MobRegistry, NpcMobUse};

/// `OpenGothic` `MAX_AI_USE_DISTANCE`, in world units (meters).
const MOB_USE_DISTANCE: f32 = MAX_AI_USE_DISTANCE;

/// Uses the nearest mob with the given `AI_UseMob` scheme.
#[derive(Component)]
#[require(ExecutingActionMarker)]
pub(crate) struct UseMobAction {
    pub(crate) scheme: StringCanon,
    pub(crate) state: i32,
}

/// Releases the current mob use when it is superseded or canceled.
pub(crate) fn start_use_mob_action(
    mut commands: Commands,
    mut npcs: Query<(Entity, &UseMobAction, Option<&NpcMobUse>), Added<UseMobAction>>,
    mut mobs: Query<&mut GameMob>,
) {
    for (entity, action, npc_mob_use) in &mut npcs {
        if action.state <= 0 {
            release_mob_occupancy(&mut commands, entity, npc_mob_use, &mut mobs);
            commands.entity(entity).remove::<ExecutingActionMarker>();
            continue;
        }
        let Some(mob_use) = npc_mob_use else {
            continue;
        };
        if mob_use.scheme == action.scheme {
            // The routine loop re-issues `AI_UseMob` until the body state
            // reports the use; drop the duplicate.
            commands.entity(entity).remove::<ExecutingActionMarker>();
            continue;
        }
        release_mob_occupancy(&mut commands, entity, Some(mob_use), &mut mobs);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn run_use_mob_action(
    mut commands: Commands,
    gothic_script: Res<ScriptResource>,
    files: Res<AnimationFiles>,
    mut handles: ResMut<CachedAssetHandles>,
    asset_server: Res<AssetServer>,
    mut desired: MessageWriter<SetDesiredAnimation>,
    mob_registry: Res<MobRegistry>,
    mut mobs: Query<(
        &mut GameMob,
        &GlobalTransform,
        Option<&MobOccupant>,
        Option<&ZenGinModelComponent>,
    )>,
    mut npcs: Query<(
        Entity,
        &GameNpc,
        &UseMobAction,
        &mut Transform,
        Option<&NpcModel>,
        Option<&NpcOverlay>,
        &mut AutoAnimSequencer,
        &mut AnimationController,
        Option<&NpcMobUse>,
        Option<&NpcBodyRootOffset>,
    )>,
) {
    let mut claimed_mobs = HashSet::new();
    for (
        entity,
        npc,
        action,
        mut transform,
        model,
        overlay,
        mut sequencer,
        mut controller,
        npc_mob_use,
        body_root_offset,
    ) in &mut npcs
    {
        let Some(model) = model else {
            continue;
        };
        if action.state <= 0 {
            if let Some(mob_use) = npc_mob_use
                && let Ok((mut mob, ..)) = mobs.get_mut(mob_use.mob)
            {
                mob.state = 0;
            }
            release_mob_use(
                &mut commands,
                entity,
                npc_mob_use,
                &mut controller,
                &mut sequencer,
            );
            commands.entity(entity).remove::<ExecutingActionMarker>();
            continue;
        }

        let Some(choice) = select_mob(
            &mut mobs,
            &mob_registry,
            &claimed_mobs,
            action.scheme.as_str(),
            transform.translation,
        ) else {
            debug!(
                "npc({}) AI_UseMob({}) found no available mob within {MOB_SEARCH_RADIUS}m",
                npc.instance_id(),
                action.scheme
            );
            commands.entity(entity).remove::<ExecutingActionMarker>();
            continue;
        };
        claimed_mobs.insert(choice.mob);

        let horizontal = horizontal_offset(transform.translation, choice.use_pos);
        if horizontal.length() > MOB_USE_DISTANCE {
            transform.rotation = face_direction_quat(horizontal);
            queue_root_motion_animation(
                &mut desired,
                entity,
                &mut controller,
                DesiredAnimation::Run,
                Some(horizontal.normalize_or_zero()),
            );
            continue;
        }

        if !choice.ready {
            // Attachment nodes are extracted once the model asset loads.
            continue;
        }
        let body_y_offset =
            body_root_offset.map_or(NPC_BODY_Y_OFFSET, |offset| -offset.0 * transform.scale.y);
        transform.translation = choice.use_pos + Vec3::Y * body_y_offset;
        transform.rotation = choice.use_rot;
        sequencer.suspend();
        controller.set_root_motion(false);
        let animation = mob_animation_name(
            model,
            overlay,
            &files,
            action.scheme.as_str(),
            choice.position_tag.as_deref(),
            action.state,
        );
        queue_named_animation(
            &mut controller,
            model,
            overlay,
            &files,
            &mut handles,
            &asset_server,
            &animation,
        );
        commands.entity(choice.mob).insert(MobOccupant(entity));
        commands.entity(entity).insert(NpcMobUse {
            mob: choice.mob,
            scheme: action.scheme.clone(),
            state: action.state.max(0),
            position_tag: choice.position_tag.clone(),
        });
        // The mob switches to the state the npc uses it in, which plays
        // its own `<STEM>-S_S<n>` animation.
        if let Ok((mut mob, ..)) = mobs.get_mut(choice.mob)
            && mob.state != action.state.max(0)
        {
            mob.state = action.state.max(0);
        }
        gothic_script.lock().call_mob_state_hook(
            npc.instance_id(),
            &choice.on_state_func,
            action.state,
        );
        // info!(
        //     "npc({}) uses mob({}) focus({}) animation({})",
        //     npc.instance_id(),
        //     choice.mob,
        //     choice.focus_name,
        //     animation
        // );
        commands.entity(entity).remove::<ExecutingActionMarker>();
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn end_use_mob_action(
    mut removed: RemovedComponents<UseMobAction>,
    mut npcs: Query<(
        Option<&NpcMobUse>,
        &mut AutoAnimSequencer,
        &mut AnimationController,
    )>,
) {
    for entity in removed.read() {
        let Ok((npc_mob_use, mut sequencer, mut controller)) = npcs.get_mut(entity) else {
            continue;
        };
        // The action committed the mob use, the pose belongs to `NpcMobUse` now.
        if npc_mob_use.is_some() {
            continue;
        }
        controller.set_root_motion(false);
        queue_stand_animation(&mut sequencer);
    }
}

pub(super) fn release_mob_use(
    commands: &mut Commands,
    npc_entity: Entity,
    mob_use: Option<&NpcMobUse>,
    controller: &mut AnimationController,
    sequencer: &mut AutoAnimSequencer,
) {
    let Some(mob_use) = mob_use else {
        return;
    };
    debug!("npc({npc_entity}) released mob({})", mob_use.mob);
    commands.entity(mob_use.mob).remove::<MobOccupant>();
    commands.entity(npc_entity).remove::<NpcMobUse>();
    controller.set_root_motion(false);
    queue_stand_animation(sequencer);
}

fn release_mob_occupancy(
    commands: &mut Commands,
    npc_entity: Entity,
    mob_use: Option<&NpcMobUse>,
    mobs: &mut Query<&mut GameMob>,
) {
    let Some(mob_use) = mob_use else {
        return;
    };
    debug!("npc({npc_entity}) released mob({})", mob_use.mob);
    commands.entity(mob_use.mob).remove::<MobOccupant>();
    commands.entity(npc_entity).remove::<NpcMobUse>();
    if let Ok(mut mob) = mobs.get_mut(mob_use.mob) {
        mob.state = 0;
    }
}

/// Available mob selected for an `AI_UseMob` request.
struct MobChoice {
    mob: Entity,
    // focus_name: String,
    /// Script hook run when the npc reaches the mob's final state.
    on_state_func: String,
    use_pos: Vec3,
    use_rot: Quat,
    /// `FRONT`/`BACK` of the chosen `ZS_POS*` node, if it has one.
    position_tag: Option<String>,
    distance_squared: f32,
    /// Attachment nodes were extracted, the npc can attach.
    ready: bool,
}

/// Nearest free mob with the requested scheme within [`MOB_SEARCH_RADIUS`].
///
/// Mob requirements (`useWithItem`, `conditionFunc`) are not checked here: the
/// engine only enforces them for the player, npcs use a mob by scheme and a
/// free attachment point alone.
fn select_mob(
    mobs: &mut Query<(
        &mut GameMob,
        &GlobalTransform,
        Option<&MobOccupant>,
        Option<&ZenGinModelComponent>,
    )>,
    registry: &MobRegistry,
    claimed: &HashSet<Entity>,
    scheme: &str,
    npc_pos: Vec3,
) -> Option<MobChoice> {
    // Script strings are canonical lowercased, the registry uses the uppercased
    // scheme derived from the visual name.
    let scheme = scheme.to_uppercase();
    let mut best: Option<MobChoice> = None;
    for &mob_entity in registry.mobs(&scheme) {
        if claimed.contains(&mob_entity) {
            continue;
        }
        let Ok((mob, mob_tr, occupant, model)) = mobs.get_mut(mob_entity) else {
            continue;
        };
        if occupant.is_some() {
            continue;
        }
        let Some((use_pos, use_rot, position_tag, ready)) =
            mob_use_transform(&mob, mob_tr, npc_pos, model.is_some())
        else {
            continue;
        };
        let distance_squared = use_pos.distance_squared(npc_pos);
        if distance_squared > MOB_SEARCH_RADIUS * MOB_SEARCH_RADIUS {
            continue;
        }
        if best
            .as_ref()
            .is_none_or(|best| distance_squared < best.distance_squared)
        {
            best = Some(MobChoice {
                mob: mob_entity,
                // focus_name: mob.focus_name.clone(),
                on_state_func: mob.on_state_func.clone(),
                use_pos,
                use_rot,
                position_tag,
                distance_squared,
                ready,
            });
        }
    }
    best
}

/// World transform the npc should use for the mob, using the nearest `ZS_POS*`
/// node. Mobs without attachment nodes fall back to the vob transform.
fn mob_use_transform(
    mob: &GameMob,
    mob_tr: &GlobalTransform,
    npc_pos: Vec3,
    has_model: bool,
) -> Option<(Vec3, Quat, Option<String>, bool)> {
    let Some(positions) = mob.positions.as_ref() else {
        // A model without extracted nodes is still loading; the npc waits at
        // the vob transform and attaches once the asset is scanned.
        return Some((mob_tr.translation(), mob_tr.rotation(), None, !has_model));
    };
    if positions.is_empty() {
        return Some((mob_tr.translation(), mob_tr.rotation(), None, true));
    }
    let mut best: Option<(GlobalTransform, Option<String>, f32)> = None;
    for position in positions {
        let world = mob_tr.mul_transform(position.transform);
        let distance_squared = world.translation().distance_squared(npc_pos);
        if best
            .as_ref()
            .is_none_or(|(_, _, best_dist)| distance_squared < *best_dist)
        {
            best = Some((
                world,
                position_tag(&position.name).map(str::to_string),
                distance_squared,
            ));
        }
    }
    let (world, tag, _) = best?;
    Some((world.translation(), world.rotation(), tag, true))
}

/// `FRONT`/`BACK` animation suffix of a `ZS_POS*` node name.
fn position_tag(name: &str) -> Option<&'static str> {
    if name.ends_with("_FRONT") {
        Some("FRONT")
    } else if name.ends_with("_BACK") {
        Some("BACK")
    } else {
        None
    }
}

/// Npc-side mob animation name, e.g. `S_BENCH_S1` or `S_BEDHIGH_FRONT_S1`.
///
/// Only beds ship position-tagged poses; other mobs (e.g. `BAUMSAEGE`) have the
/// attachment node but only the untagged animation, so the tag is used only
/// when the archive has it.
fn mob_animation_name(
    model: &NpcModel,
    overlay: Option<&NpcOverlay>,
    files: &AnimationFiles,
    scheme: &str,
    position_tag: Option<&str>,
    state: i32,
) -> String {
    let state = state.max(1);
    let scheme = scheme.to_uppercase();
    let tagged = position_tag.map(|tag| format!("S_{scheme}_{tag}_S{state}"));
    if let Some(tagged) = tagged
        && has_animation(model, overlay, files, &tagged)
    {
        return tagged;
    }
    format!("S_{scheme}_S{state}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Selected(Option<Entity>);

    fn select(mut selected: ResMut<Selected>, registry: Res<MobRegistry>, mut mobs: MobQuery) {
        let claimed = HashSet::new();
        selected.0 = select_mob(&mut mobs, &registry, &claimed, "bsanvil", Vec3::ZERO)
            .map(|choice| choice.mob);
    }

    type MobQuery<'w, 's> = Query<
        'w,
        's,
        (
            &'static mut GameMob,
            &'static GlobalTransform,
            Option<&'static MobOccupant>,
            Option<&'static ZenGinModelComponent>,
        ),
    >;

    fn app_with_mob(use_with_item: &str, occupant: Option<Entity>) -> (App, Entity) {
        let mut app = App::new();
        app.init_resource::<MobRegistry>();
        app.init_resource::<Selected>();
        app.add_systems(Update, select);

        let mut mob = app.world_mut().spawn((
            game_mob("BSANVIL", use_with_item),
            Transform::IDENTITY,
            GlobalTransform::IDENTITY,
        ));
        if let Some(occupant) = occupant {
            mob.insert(MobOccupant(occupant));
        }
        let mob = mob.id();
        app.world_mut()
            .resource_mut::<MobRegistry>()
            .insert("BSANVIL".to_string(), mob);
        app.update();
        (app, mob)
    }

    fn game_mob(scheme: &str, use_with_item: &str) -> GameMob {
        GameMob {
            vob_name: String::new(),
            focus_name: String::new(),
            scheme: scheme.to_string(),
            visual_stem: scheme.to_uppercase(),
            state_num: 1,
            use_with_item: use_with_item.to_string(),
            on_state_func: String::new(),
            condition_func: String::new(),
            rewind: false,
            trigger_target: String::new(),
            trigger_fired: false,
            positions: None,
            state: 0,
        }
    }

    #[test]
    fn select_mob_ignores_the_use_item_requirement() {
        // The engine only enforces `useWithItem` for the player, an npc uses an
        // anvil without carrying the hot blank.
        let (app, mob) = app_with_mob("ItMiSwordRawHot", None);

        assert_eq!(app.world().resource::<Selected>().0, Some(mob));
    }

    #[test]
    fn select_mob_skips_an_occupied_mob() {
        let (app, _) = app_with_mob("", Some(Entity::PLACEHOLDER));

        assert_eq!(app.world().resource::<Selected>().0, None);
    }

    #[test]
    fn mob_animation_uses_the_position_tag_only_when_it_exists() {
        let model = NpcModel {
            animation_scheme: Some("HUMANS".to_string()),
            ..Default::default()
        };

        let beds = AnimationFiles(HashSet::from(["HUMANS-S_BEDHIGH_BACK_S1.MAN".to_string()]));
        assert_eq!(
            mob_animation_name(&model, None, &beds, "BEDHIGH", Some("BACK"), 1),
            "S_BEDHIGH_BACK_S1"
        );

        // The sawmill has a `_BACK` attachment node but only an untagged pose.
        let sawmill = AnimationFiles(HashSet::from(["HUMANS-S_BAUMSAEGE_S1.MAN".to_string()]));
        assert_eq!(
            mob_animation_name(&model, None, &sawmill, "BAUMSAEGE", Some("BACK"), 1),
            "S_BAUMSAEGE_S1"
        );
    }
}
