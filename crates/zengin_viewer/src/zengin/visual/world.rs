//! Loads the world zen: the world mesh, static vob objects, lights, items,
//! spots and the way net into [`ZenGinWorldData`].

use bevy::prelude::*;

use std::collections::HashMap;

use zen_kit_rs::{
    misc::{GameVersion, VfsOverwriteBehavior, VisualType, VobType},
    vfs::Vfs,
    vobs::virtual_object::VirtualObject,
    waynet::WayNet,
};

use crate::{
    warn_unimplemented,
    zengin::{
        common::*,
        loaders::{ZenginMat3ToGlam, ZenginVec3ToGlam, vfs::find_mesh_path},
        script::state::SpawnItem,
        script::symbol_id::InstanceId,
        visual::mesh::meshes_from_zengin_mesh,
        waypoint_graph::{WaypointGraph, WaypointNode},
    },
};

const PLARCEHOLDER_MESH: &str = "/_WORK/DATA/MESHES/_COMPILED/SPHERE.MRM";

pub fn load_zengin_world_data(
    world_path: &str,
    vm_state: &crate::zengin::script::state::LoadedState,
    spawn_item: &mut MessageWriter<SpawnItem>,
) -> ZenGinWorldData {
    let vfs = Vfs::new();

    // TODO use shared vfs which is also used by zengin loader
    let vfs_override = VfsOverwriteBehavior::ALL;
    let dir = gothic2_dir();
    vfs.mount_disk_host(&format!("{}/Data/Anims.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Anims_Addon.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Meshes.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Meshes_Addon.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Sounds.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Sounds_Addon.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Sounds_bird_01.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Speech1.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Speech2.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Speech_Addon.vdf", dir), vfs_override);
    vfs.mount_disk_host(
        &format!("{}/Data/Speech_English_Patch_Atari.vdf", dir),
        vfs_override,
    );
    vfs.mount_disk_host(
        &format!("{}/Data/Speech_heyou_citygde_engl.vdf", dir),
        vfs_override,
    );
    vfs.mount_disk_host(
        &format!("{}/Data/Speech_Parlan_engl.vdf", dir),
        vfs_override,
    );
    vfs.mount_disk_host(&format!("{}/Data/SystemPack.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Textures.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Textures_Addon.vdf", dir), vfs_override);
    vfs.mount_disk_host(
        &format!("{}/Data/Textures_Addon_Menu_English.vdf", dir),
        vfs_override,
    );
    vfs.mount_disk_host(
        &format!("{}/Data/Textures_Fonts_Apostroph.vdf", dir),
        vfs_override,
    );
    vfs.mount_disk_host(
        &format!("{}/Data/Textures_multilingual_Jowood.vdf", dir),
        vfs_override,
    );
    vfs.mount_disk_host(&format!("{}/Data/Worlds.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Worlds_Addon.vdf", dir), vfs_override);

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
    let item_ids: HashMap<&str, InstanceId> = vm_state
        .items_instances
        .iter()
        .map(|(id, item)| (item.name.as_str(), *id))
        .collect();

    for obj in world.root_objects() {
        load_objects(&vfs, &mut data, &item_ids, &obj, spawn_item);
    }
    let way_net = world.way_net();
    load_way_net_points(&way_net, &mut data);

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

fn load_way_net_points(way_net: &WayNet, data: &mut ZenGinWorldData) {
    let points = way_net.points();
    let mut graph = WaypointGraph::default();
    for point in &points {
        let name = point.name().to_lowercase();
        let pos = get_world_pos(point.position().to_glam());
        let direction = point.direction();
        let dir_2d = Vec2 {
            x: direction.x,
            y: direction.z,
        };
        let rot = get_world_dir_quat(dir_2d);
        let tr = Transform::from_translation(pos).with_rotation(rot);
        data.way_points.insert(name.clone(), tr);

        let index = graph.nodes.len();
        graph.name_to_index.insert(name.clone(), index);
        graph.nodes.push(WaypointNode {
            // name,
            position: pos,
            // direction: Vec3::new(direction.x, direction.y, direction.z),
            free: point.free_point(),
        });
    }

    // Edge endpoints are copies of the point values, not the same objects, so
    // match by the (unique) lowercased waypoint name
    graph.adjacency = vec![Vec::new(); graph.nodes.len()];
    for edge in way_net.edges() {
        let start = graph
            .name_to_index
            .get(&edge.start_point().name().to_lowercase())
            .copied();
        let end = graph
            .name_to_index
            .get(&edge.end_point().name().to_lowercase())
            .copied();
        let (Some(start), Some(end)) = (start, end) else {
            continue;
        };
        let length = (graph.nodes[start].position - graph.nodes[end].position).length();
        graph.adjacency[start].push((end, length));
        graph.adjacency[end].push((start, length));
    }

    data.waypoint_graph = graph;
    let edges = data
        .waypoint_graph
        .adjacency
        .iter()
        .map(Vec::len)
        .sum::<usize>()
        / 2;
    info!(
        "Waypoint graph: {} nodes, {} edges",
        data.waypoint_graph.nodes.len(),
        edges
    );
}

fn load_objects(
    vfs: &Vfs,
    data: &mut ZenGinWorldData,
    item_ids: &HashMap<&str, InstanceId>,
    obj: &VirtualObject,
    spawn_item: &mut MessageWriter<SpawnItem>,
) {
    let visual = obj.visual();
    let visual_name = visual.name();
    let visual_type = visual.get_type();

    if !visual_name.is_empty() && obj.show_visual() {
        assert!(obj.get_type() != VobType::zCVobLight);
        let asset_path = match visual_type {
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
        };

        if let Some(asset_path) = asset_path {
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
    item_ids: &HashMap<&str, InstanceId>,
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
        | VobType::oCMobContainer
        | VobType::oCMobDoor
        | VobType::oCCSTrigger
        | VobType::zCVobStartpoint
        | VobType::unknown => {
            panic!();
        }
    }
    // println!("VOB kind({type_:?}), name({name}) not handled");
}
