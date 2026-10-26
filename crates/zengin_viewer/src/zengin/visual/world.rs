//! Loads the world zen: the world mesh, static vob objects, lights, items,
//! spots and the way net into [`ZenGinWorldData`].

use bevy::prelude::*;

use std::collections::HashMap;

use zen_kit_rs::{
    misc::{GameVersion, VisualType, VobType},
    vfs::Vfs,
    vobs::virtual_object::{VirtualObject, Visual},
};

use crate::{
    warn_unimplemented,
    zengin::{
        common::*,
        loaders::{ZenginMat3ToGlam, ZenginVec3ToGlam, vfs::find_mesh_path},
        script::state::SpawnItem,
        script::symbol_id::InstanceId,
        visual::mesh::meshes_from_zengin_mesh,
        waypoint_graph::load_way_net_points,
    },
};

const PLARCEHOLDER_MESH: &str = "/_WORK/DATA/MESHES/_COMPILED/SPHERE.MRM";

pub fn load_zengin_world_data(
    vfs: &Vfs,
    world_path: &str,
    vm_state: &crate::zengin::script::state::LoadedState,
    spawn_item: &mut MessageWriter<SpawnItem>,
) -> ZenGinWorldData {
    if false {
        print_nodes(&vfs.get_root(), 0);
    }

    let world_node = vfs.resolve_path(world_path).unwrap();
    let world_read = world_node.open().unwrap();
    let world =
        zen_kit_rs::world::World::load_versioned(&world_read, GameVersion::GOTHIC2).unwrap();

    let world_mesh = world.mesh();
    let world_bevy_meshes = meshes_from_zengin_mesh(&world_mesh);

    let mut data = ZenGinWorldData {
        world_model: world_bevy_meshes,
        ..Default::default()
    };

    // Item instance name -> instance id, resolves world vob and armor item names
    let item_ids: HashMap<String, InstanceId> = vm_state
        .items_instances
        .iter()
        .map(|(id, item)| (item.name.to_lowercase(), *id))
        .collect();

    for obj in world.root_objects() {
        load_objects(vfs, &mut data, &item_ids, &obj, spawn_item);
    }
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

    // Names of all anim files, used to resolve overlay animations
    if let Some(dir) = vfs.resolve_path("/_WORK/DATA/ANIMS/_COMPILED") {
        for child in dir.enumerate_children() {
            data.anim_files.insert(child.name().to_uppercase());
        }
    }

    data
}

fn load_objects(
    vfs: &Vfs,
    data: &mut ZenGinWorldData,
    item_ids: &HashMap<String, InstanceId>,
    obj: &VirtualObject,
    spawn_item: &mut MessageWriter<SpawnItem>,
) {
    let visual = obj.visual();
    let has_visual = !visual.name().is_empty() && obj.show_visual();

    if obj.get_type() == VobType::oCMobContainer {
        handle_container(vfs, data, item_ids, obj, has_visual);
    } else if has_visual {
        assert_ne!(obj.get_type(), VobType::zCVobLight);
        if let Some(asset_path) = visual_asset_path(vfs, &visual) {
            let tr = get_obj_tr(obj);
            data.static_models.push(ZenGinInstance {
                tr,
                archetype: asset_path,
                collides: obj.cd_dynamic(),
            });
        }
    } else {
        handle_other_vob(data, item_ids, obj, spawn_item);
    }
    for child in obj.children() {
        load_objects(vfs, data, item_ids, &child, spawn_item);
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

fn handle_container(
    vfs: &Vfs,
    data: &mut ZenGinWorldData,
    item_ids: &HashMap<String, InstanceId>,
    obj: &VirtualObject,
    has_visual: bool,
) {
    let archetype = has_visual
        .then(|| visual_asset_path(vfs, &obj.visual()))
        .flatten()
        .unwrap();
    data.chests.push(ZenGinChest {
        tr: get_obj_tr(obj),
        archetype,
        collides: obj.cd_dynamic(),
        collider_half_extents: chest_collider_half_extents(obj),
        name: obj.movable_name(),
        items: container_items(&obj.container_contents(), item_ids),
        locked: obj.container_locked(),
    });
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

fn chest_collider_half_extents(obj: &VirtualObject) -> Vec3 {
    let bbox = obj.bbox();
    let size = (bbox.max.to_glam() - bbox.min.to_glam()) / ZENGIN_UNITS_PER_WORLD_UNIT;
    let half = size * 0.5;
    if half.is_finite() {
        half.max(Vec3::splat(0.2)).min(Vec3::splat(2.0))
    } else {
        Vec3::splat(0.5)
    }
}

fn get_obj_tr(obj: &VirtualObject) -> Transform {
    let pos = get_world_pos(obj.position().to_glam());
    let rot = get_world_rot(obj.rotation().to_glam());
    return Transform::from_translation(pos).with_rotation(rot);
}

fn handle_light(obj: &VirtualObject, data: &mut ZenGinWorldData) {
    let pos = get_world_pos(obj.position().to_glam());
    let rot = get_world_rot(obj.rotation().to_glam());
    // if !obj.cd_dynamic() {
    //     return;
    // }
    data.light_instances.push(LightInstance { pos, rot });
}

fn handle_other_vob(
    data: &mut ZenGinWorldData,
    item_ids: &HashMap<String, InstanceId>,
    obj: &VirtualObject,
    spawn_item: &mut MessageWriter<SpawnItem>,
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

            spawn_item.write(SpawnItem {
                instance_id,
                location_name: None,
                transform: Some(tr),
            });

            return;
        }
        VobType::zCVobLight => {
            handle_light(obj, data);
            return;
        }
        VobType::zCVobSpot => {
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
            warn_unimplemented!("VobType::zCTriggerWorldStart");
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
        VobType::oCMobInter => {
            warn_unimplemented!("VobType::oCMobInter");
        }
        VobType::oCMobSwitch => {
            warn_unimplemented!("VobType::oCMobSwitch");
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
            warn_unimplemented!("VobType::oCTriggerChangeLevel");
        }
        VobType::zCMover => {
            warn_unimplemented!("VobType::zCMover");
        }

        VobType::zCVobSound
        | VobType::zCVobSoundDaytime
        | VobType::oCZoneMusic
        | VobType::oCZoneMusicDefault => {
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
        VobType::ignored => {
            return;
        }
        // These should be handled in other function
        VobType::oCMobContainer => {
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
        | VobType::zCVobStartpoint
        | VobType::unknown => {
            panic!();
        }
    }
    // println!("VOB kind({type_:?}), name({name}) not handled");
}
