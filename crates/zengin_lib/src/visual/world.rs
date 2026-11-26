//! Loads the world zen: the world mesh, static vob objects, items,
//! spots and the way net into [`ZenGinWorldData`].

use bevy::prelude::*;
use bevy::shape::Aabb3d;

use std::collections::HashMap;

use zen_kit_rs::{
    misc::{GameVersion, VisualType, VobType},
    vfs::Vfs,
    vobs::virtual_object::{VirtualObject, Visual},
};

use crate::{
    MobBase,
    common::*,
    loaders::{ZenginMat3ToGlam, ZenginQuatToGlam, ZenginVec3ToGlam, vfs::find_mesh_path},
    script::state::SpawnItem,
    script::symbol_id::InstanceId,
    visual::mesh::meshes_from_zengin_mesh,
    waypoint_graph::load_way_net_points,
    world_spawn::{
        ContainerSpawn, InteractiveSpawn, MobSpawnKind, SpawnMob, SpawnMover, SpawnStaticModel,
        SpawnWorldMesh, VobPrototype, WorldSpawnMessages,
    },
};
use dev_util::warn_unimplemented;

const PLARCEHOLDER_MESH: &str = "/_WORK/DATA/MESHES/_COMPILED/SPHERE.MRM";

pub fn load_zengin_world_data(
    vfs: &Vfs,
    world_path: &str,
    vm_state: &crate::script::state::LoadedState,
    spawns: &WorldSpawnMessages,
    spawn_mesh: bool,
) -> ZenGinWorldData {
    if false {
        print_nodes(&vfs.get_root(), 0);
    }

    let world_node = vfs.resolve_path(world_path).unwrap();
    let world_read = world_node.open().unwrap();
    let world =
        zen_kit_rs::world::World::load_versioned(&world_read, GameVersion::GOTHIC2).unwrap();

    let mut data = ZenGinWorldData::default();
    if spawn_mesh {
        let world_mesh = world.mesh();
        let world_bevy_meshes = meshes_from_zengin_mesh(&world_mesh);
        spawns.world_mesh.send(SpawnWorldMesh {
            model: world_bevy_meshes,
        });
    }

    // Item instance name -> instance id, resolves world vob and armor item names
    let item_ids: HashMap<String, InstanceId> = vm_state
        .items_instances
        .iter()
        .map(|(id, item)| {
            let item = item
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (item.name.to_lowercase(), *id)
        })
        .collect();

    for obj in world.root_objects() {
        load_objects(vfs, &mut data, &item_ids, &obj);
    }
    info!(
        "Loaded {} music zones (default: {:?})",
        data.music_zones.len(),
        data.default_music_zone
            .as_ref()
            .map(|zone| zone.name.as_str())
    );
    let way_net = world.way_net();
    let (waypoint_graph, way_points) = load_way_net_points(&way_net);
    data.waypoint_graph = waypoint_graph;
    data.way_points = way_points;

    if !world.npcs().is_empty() {
        warn_unimplemented!("Loading NPCs from world");
    }

    if world.spawn_location_count() > 0 {
        warn_unimplemented!("Handling spawn locations");
    }

    data
}

fn load_objects(
    vfs: &Vfs,
    data: &mut ZenGinWorldData,
    item_ids: &HashMap<String, InstanceId>,
    obj: &VirtualObject,
) {
    let name = obj.name();
    if !name.is_empty() {
        data.names.entry(name.to_lowercase()).or_insert(obj.id());
    }
    let visual = obj.visual();
    let has_visual = !visual.name().is_empty() && obj.show_visual();

    if obj.get_type() == VobType::zCMover {
        handle_mover(vfs, obj, has_visual, data);
    } else if obj.get_type() == VobType::oCMobContainer {
        handle_container(vfs, item_ids, obj, has_visual, data);
    } else if is_interactive_mob(obj.get_type()) {
        handle_mob(vfs, obj, data);
    } else if has_visual {
        assert_ne!(obj.get_type(), VobType::zCVobLight);
        if let Some(asset_path) = visual_asset_path(vfs, &visual) {
            data.prototypes.insert(
                obj.id(),
                VobPrototype::Static(SpawnStaticModel {
                    vob_id: obj.id(),
                    tr: get_obj_tr(obj),
                    archetype: asset_path,
                    collides: obj.cd_dynamic(),
                }),
            );
        }
    } else {
        handle_other_vob(vfs, data, item_ids, obj);
    }
    for child in obj.children() {
        load_objects(vfs, data, item_ids, &child);
    }
}

fn visual_asset_path(vfs: &Vfs, visual: &Visual<'_>) -> Option<String> {
    let visual_name = visual.name();
    match visual.get_type() {
        VisualType::MULTI_RESOLUTION_MESH
        | VisualType::MODEL
        | VisualType::MORPH_MESH
        | VisualType::MESH => {
            if let Some(asset_path) = find_mesh_path(vfs, &visual_name) {
                Some(asset_path)
            } else {
                println!("Failed to find mesh for visual({})", visual_name);
                Some(PLARCEHOLDER_MESH.to_string())
            }
        }
        VisualType::DECAL => {
            warn_unimplemented!("load VisualType::DECAL");
            None
        }
        VisualType::PARTICLE_EFFECT => {
            warn_unimplemented!("load VisualType::PARTICLE_EFFECT");
            None
        }
        VisualType::CAMERA => {
            warn_unimplemented!("load VisualType::CAMERA");
            None
        }
        VisualType::UNKNOWN => {
            warn_unimplemented!("load VisualType::UNKNOWN");
            None
        }
    }
}

/// Interactive mob classes driven by `AI_UseMob`; `oCMobContainer` has its
/// own loot flow.
fn is_interactive_mob(type_: VobType) -> bool {
    matches!(
        type_,
        VobType::oCMobInter
            | VobType::oCMobBed
            | VobType::oCMobFire
            | VobType::oCMobLadder
            | VobType::oCMobSwitch
            | VobType::oCMobWheel
            | VobType::oCMobDoor
    )
}

fn handle_mob(vfs: &Vfs, obj: &VirtualObject, data: &mut ZenGinWorldData) {
    let visual = obj.visual();
    let visual_name = visual.name();
    // The engine loads the mesh of a `showVisual=false` mob as well, it only
    // skips drawing it. Keep the model to read the `ZS_POS*` attachment nodes.
    let archetype = (!visual_name.is_empty())
        .then(|| visual_asset_path(vfs, &visual))
        .flatten()
        .unwrap_or_default();
    let scheme = mob_scheme(&visual_name);
    // Switch-like vobs end the player's use automatically; the scheme fallback
    // covers mods that use an `oCMobInter` class for a lever/button visual.
    let auto_end = matches!(
        obj.get_type(),
        VobType::oCMobSwitch | VobType::oCMobWheel | VobType::oCMobDoor
    ) || is_switch_mob_scheme(&scheme);
    let no_move =
        matches!(obj.get_type(), VobType::oCMobDoor) || scheme.eq_ignore_ascii_case("DOOR");
    data.prototypes.insert(
        obj.id(),
        VobPrototype::Mob(SpawnMob {
            base: MobBase {
                vob_id: obj.id(),
                vob_name: obj.name(),
                focus_name: obj.movable_name(),
            },
            tr: get_obj_tr(obj),
            archetype,
            visual_hidden: !obj.show_visual(),
            collides: obj.cd_dynamic(),
            scheme: scheme.clone(),
            visual_stem: mob_visual_stem(&visual_name),
            state_num: obj.interactive_state(),
            kind: MobSpawnKind::Interactive(InteractiveSpawn {
                use_with_item: obj.interactive_use_with_item(),
                on_state_func: obj.interactive_on_state_change_function(),
                condition_func: obj.interactive_condition_function(),
                rewind: obj.interactive_rewind(),
                trigger_target: obj.interactive_target(),
                auto_end,
                no_move,
            }),
        }),
    );
}

fn handle_mover(vfs: &Vfs, obj: &VirtualObject, has_visual: bool, data: &mut ZenGinWorldData) {
    let visual = obj.visual();
    let archetype = has_visual
        .then(|| visual_asset_path(vfs, &visual))
        .flatten()
        .unwrap_or_default();
    let keyframes = obj
        .mover_keyframes()
        .into_iter()
        .map(|keyframe| {
            Transform::from_translation(get_world_pos(keyframe.position.to_glam()))
                // Keyframe quaternions are stored transposed relative to the
                // vob rotation matrix, so frame 0 would otherwise not match the
                // vob's rest pose and the mover would rotate around a wrong
                // axis.
                .with_rotation(get_world_quat(keyframe.rotation.to_glam().conjugate()))
        })
        .collect();
    data.prototypes.insert(
        obj.id(),
        VobPrototype::Mover(SpawnMover {
            vob_id: obj.id(),
            tr: get_obj_tr(obj),
            archetype,
            collides: obj.cd_dynamic(),
            name: obj.name(),
            keyframes,
            // Archive speed is in zengin units per millisecond.
            speed: obj.mover_speed() * 1000.0 / ZENGIN_UNITS_PER_WORLD_UNIT,
            start_enabled: obj.trigger_start_enabled(),
            behavior: obj.mover_behavior(),
        }),
    );
}

fn handle_container(
    vfs: &Vfs,
    item_ids: &HashMap<String, InstanceId>,
    obj: &VirtualObject,
    has_visual: bool,
    data: &mut ZenGinWorldData,
) {
    let visual = obj.visual();
    let visual_name = visual.name();
    let archetype = has_visual
        .then(|| visual_asset_path(vfs, &visual))
        .flatten()
        .unwrap();
    let vob_id = obj.id();
    let items = container_items(&obj.container_contents(), item_ids);
    data.prototypes.insert(
        vob_id,
        VobPrototype::Mob(SpawnMob {
            base: MobBase {
                vob_id,
                vob_name: obj.name(),
                focus_name: obj.movable_name(),
            },
            tr: get_obj_tr(obj),
            archetype,
            visual_hidden: false,
            collides: obj.cd_dynamic(),
            scheme: mob_scheme(&visual_name),
            visual_stem: mob_visual_stem(&visual_name),
            state_num: obj.interactive_state(),
            kind: MobSpawnKind::Container(ContainerSpawn {
                items,
                locked: obj.container_locked(),
                key: obj.container_key(),
                pick_string: obj.container_pick_string(),
                cracked: false,
            }),
        }),
    );
}

/// Parses the `oCMobContainer.contents` string: `itemInstance[:count]` tokens
/// separated by commas or semicolons.
fn container_items(
    contents: &str,
    item_ids: &HashMap<String, InstanceId>,
) -> Vec<(InstanceId, i32)> {
    contents
        .split([',', ';'])
        .filter_map(|token| fun_name(item_ids, token))
        .collect()
}

fn fun_name(item_ids: &HashMap<String, InstanceId>, token: &str) -> Option<(InstanceId, i32)> {
    let token = token.trim();
    if token.is_empty() {
        return None;
    }
    let (name, count) = token.split_once(':').unwrap_or((token, "1"));
    let count = if let Ok(count) = count.trim().parse::<i32>() {
        count
    } else {
        warn!("Invalid item count in container contents({token})");
        1
    };
    if count <= 0 {
        return None;
    }
    let name = name.trim().to_lowercase();
    let Some(instance_id) = item_ids.get(name.as_str()).copied() else {
        warn!("Not found instance id for container item({name})");
        return None;
    };
    Some((instance_id, count))
}

/// `Newworld\DragonIsland.zen` -> `Newworld/DragonIsland`.
fn change_level_world_name(level: &str) -> String {
    let path = level.replace('\\', "/");
    let path = path.trim().trim_matches('/');
    match path.rsplit_once('.') {
        Some((stem, extension)) if extension.eq_ignore_ascii_case("zen") => {
            stem.trim_end_matches('/').to_string()
        }
        _ => path.to_string(),
    }
}

fn change_level_trigger(vfs: &Vfs, obj: &VirtualObject, name: &str) -> Option<ChangeLevelTrigger> {
    let level = change_level_world_name(&obj.change_level_name());
    if level.is_empty() {
        warn!("oCTriggerChangeLevel vob({name}) without a level name");
        return None;
    }
    let world_path = format!("/_WORK/DATA/WORLDS/{level}.ZEN");
    if vfs.resolve_path(&world_path).is_none() {
        warn!("oCTriggerChangeLevel vob({name}) targets missing world({world_path})");
        return None;
    }
    let bbox = obj.bbox();
    let corner_a = get_world_pos(bbox.min.to_glam());
    let corner_b = get_world_pos(bbox.max.to_glam());
    Some(ChangeLevelTrigger {
        name: name.to_string(),
        level,
        start_vob: obj.change_level_start_vob(),
        bounds: Aabb3d::from_min_max(corner_a.min(corner_b), corner_a.max(corner_b)),
        start_enabled: obj.trigger_start_enabled(),
        respond_to_pc: obj.trigger_respond_to_pc(),
    })
}

fn get_obj_tr(obj: &VirtualObject) -> Transform {
    let pos = get_world_pos(obj.position().to_glam());
    let rot = get_world_rot(obj.rotation().to_glam());
    return Transform::from_translation(pos).with_rotation(rot);
}

/// Converts a music zone vob's bounds to world units. Degenerate boxes are
/// kept as-is; the half-open containment test never matches them.
fn music_zone(obj: &VirtualObject) -> Option<MusicZone> {
    let name = obj.name();
    if name.is_empty() {
        warn!("Music zone vob without a name, skipping");
        return None;
    }

    let bbox = obj.bbox();
    let corner_a = get_world_pos(bbox.min.to_glam());
    let corner_b = get_world_pos(bbox.max.to_glam());
    Some(MusicZone {
        name,
        bounds: bevy::shape::Aabb3d::from_min_max(corner_a.min(corner_b), corner_a.max(corner_b)),
    })
}

fn handle_other_vob(
    vfs: &Vfs,
    data: &mut ZenGinWorldData,
    item_ids: &HashMap<String, InstanceId>,
    obj: &VirtualObject,
) {
    let name = obj.name();
    let type_ = obj.get_type();
    match type_ {
        VobType::oCItem => {
            let tr = get_obj_tr(obj);
            let name = name.to_lowercase();

            let Some(instance_id) = item_ids.get(name.as_str()).copied() else {
                warn!("Not found instance id for item({}) vob", name);
                return;
            };

            data.prototypes.insert(
                obj.id(),
                VobPrototype::Item(SpawnItem {
                    instance_id,
                    transform: Some(tr),
                }),
            );

            return;
        }
        VobType::zCVobLight | VobType::ignored => {
            // Light vobs only carry baked lighting, runtime lights are not spawned.
            return;
        }
        VobType::zCVobSpot | VobType::zCVobStartpoint => {
            let tr = get_obj_tr(obj);
            data.spots.insert(name.to_lowercase().clone(), tr);
            return;
        }
        VobType::zCVob => {
            warn_unimplemented!("VobType::zCVob");
        }
        VobType::zCVobLevelCompo => {
            warn_unimplemented!("VobType::zCVobLevelCompo");
        }
        VobType::zCPFXController => {
            warn_unimplemented!("VobType::zCPFXControlle");
        }

        VobType::zCMessageFilter => {
            warn_unimplemented!("VobType::zCMessageFilter");
        }
        VobType::zCCodeMaster => {
            warn_unimplemented!("VobType::zCCodeMaster");
        }
        VobType::zCTriggerWorldStart => {
            let target = obj.world_start_target();
            if target.is_empty() {
                warn!("zCTriggerWorldStart vob({}) without a target", obj.name());
                return;
            }
            data.world_start_triggers.push(WorldStartTrigger {
                target,
                fire_only_first_time: obj.world_start_fire_once(),
            });
            return;
        }
        VobType::zCCSCamera => {
            warn_unimplemented!("VobType::zCCSCamera");
        }
        VobType::zCCamTrj_KeyFrame => {
            warn_unimplemented!("VobType::zCCamTrj_KeyFrame");
        }
        VobType::oCTouchDamage => {
            warn_unimplemented!("VobType::oCTouchDamage");
        }
        VobType::zCTriggerUntouch => {
            warn_unimplemented!("VobType::zCTriggerUntouch");
        }
        VobType::zCEarthquake => {
            warn_unimplemented!("VobType::zCEarthquake");
        }

        VobType::zCTrigger => {
            warn_unimplemented!("VobType::zCTrigger");
        }
        VobType::zCTriggerList => {
            warn_unimplemented!("VobType::zCTriggerList");
        }
        VobType::oCTriggerScript => {
            warn_unimplemented!("VobType::oCTriggerScript");
        }
        VobType::oCTriggerChangeLevel => {
            if let Some(trigger) = change_level_trigger(vfs, obj, &name) {
                data.change_level_triggers.push(trigger);
            }
            return;
        }
        VobType::zCMover => {
            warn_unimplemented!("VobType::zCMover");
        }

        VobType::oCZoneMusic => {
            if let Some(zone) = music_zone(obj) {
                data.music_zones.push(zone);
            }
            return;
        }
        VobType::oCZoneMusicDefault => {
            if let Some(zone) = music_zone(obj) {
                data.default_music_zone = Some(zone);
            }
            return;
        }
        VobType::zCVobSound | VobType::zCVobSoundDaytime => {
            warn_unimplemented!("Sound Vobs unimplemented");
            return;
        }

        VobType::zCZoneZFog => {
            warn_unimplemented!("VobType::zCZoneZFog");
        }
        VobType::zCZoneZFogDefault => {
            warn_unimplemented!("VobType::zCZoneZFogDefault");
        }
        VobType::zCZoneVobFarPlane => {
            warn_unimplemented!("VobType::zCZoneVobFarPlane");
        }
        VobType::zCZoneVobFarPlaneDefault => {
            warn_unimplemented!("VobType::zCZoneVobFarPlaneDefault");
        }
        VobType::zCVobLensFlare => {
            warn_unimplemented!("VobType::zCVobLensFlare");
        }
        // These should be handled in other functions
        VobType::oCMobContainer | VobType::oCMobInter | VobType::oCMobSwitch => {
            panic!("should be handled in other function");
        }
        // These are not used in NEWWORLD.ZEN
        VobType::oCNpc
        | VobType::zCMoverController
        | VobType::zCVobScreenFX
        | VobType::zCVobStair
        | VobType::zCVobAnimate
        | VobType::oCMOB
        | VobType::oCMobBed
        | VobType::oCMobFire
        | VobType::oCMobLadder
        | VobType::oCMobWheel
        | VobType::oCMobDoor
        | VobType::oCCSTrigger
        | VobType::unknown => {
            panic!();
        }
    }
    // println!("VOB kind({type_:?}), name({name}) not handled");
}

#[cfg(test)]
mod tests {
    use super::change_level_world_name;

    #[test]
    fn change_level_names_are_normalized_to_world_paths() {
        assert_eq!(
            change_level_world_name("ADDON\\ADDONWORLD.ZEN"),
            "ADDON/ADDONWORLD"
        );
        assert_eq!(
            change_level_world_name("Newworld\\DragonIsland.zen"),
            "Newworld/DragonIsland"
        );
        assert_eq!(
            change_level_world_name("OLDWORLD/OLDWORLD.ZEN"),
            "OLDWORLD/OLDWORLD"
        );
        assert_eq!(
            change_level_world_name("  OLDWORLD/OLDWORLD.zen  "),
            "OLDWORLD/OLDWORLD"
        );
        assert_eq!(change_level_world_name(""), "");
    }
}
