use std::f32::consts::PI;

use bevy::prelude::*;

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
        script::script_vm::{InstanceState, SpawnNpc},
        visual::mesh::meshes_from_gothic_mesh,
    },
};

const PLARCEHOLDER_MESH: &str = "/_WORK/DATA/MESHES/_COMPILED/SPHERE.MRM";
const HUMAN_MODEL: &str = "/_WORK/DATA/ANIMS/_COMPILED/HUM_BODY_NAKED0.MDM";

pub fn load_npc(instance: &InstanceState, spawn_npc: &SpawnNpc, data: &mut ZenGinWorldData) {
    let way_point_name = spawn_npc
        .routine_way_point
        .as_ref()
        .unwrap_or(&spawn_npc.way_point);
    let Some(tr) = data.way_points.get(way_point_name) else {
        warn_once!("some way points are not found, example({})", way_point_name);
        return;
    };

    warn_once!("Placing human NPC is hacks and hardcoding");
    let tr = tr.to_matrix();
    let tr = Transform::from_translation(Vec3 {
        x: 0.0,
        y: -0.5,
        z: 0.0,
    })
    .to_matrix()
        * tr;

    let tr_body = tr;

    let head_dt = Vec3 {
        x: 0.00,
        y: 1.62,
        z: 0.045,
    };
    let head_rot = Quat::from_axis_angle(
        Vec3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
        -PI / 2.0,
    );
    let head_transform = tr;
    let head_transform = head_transform * Transform::from_translation(head_dt).to_matrix();
    let head_transform = head_transform * Mat4::from_quat(head_rot);

    let model = format!(
        "/_WORK/DATA/ANIMS/_COMPILED/{}.MMB",
        instance.head_model.to_uppercase()
    );
    let head_model = to_asset_path(&model);
    let body_model = to_asset_path(HUMAN_MODEL);
    let npc = ZenGinNpc {
        head_tr: Transform::from_matrix(head_transform),
        head_model,
        head_texture: get_full_texture_path(&instance.face_texture),
        body_tr: Transform::from_matrix(tr_body),
        body_model,
        body_texture: get_full_texture_path(&instance.body_texture),
    };
    data.npcs.push(npc);
}

pub fn load_gothic_world_data(
    world_path: &str,
    vm_state: &crate::zengin::script::script_vm::State,
) -> ZenGinWorldData {
    let vfs = Vfs::new();

    let vfs_override = VfsOverwriteBehavior::ALL;
    let dir = gothic2_dir();
    vfs.mount_disk_host(&format!("{}/Data/Worlds.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Textures.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Meshes.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Meshes_Addon.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Anims.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/Anims_Addon.vdf", dir), vfs_override);
    vfs.mount_disk_host(&format!("{}/Data/SystemPack.vdf", dir), vfs_override);

    if false {
        print_nodes(&vfs.get_root(), 0);
    }

    let world_node = vfs.resolve_path(world_path).unwrap();
    let world_read = world_node.open().unwrap();
    let world =
        zen_kit_rs::world::World::load_versioned(&world_read, GameVersion::GOTHIC2).unwrap();

    let world_mesh = world.mesh();
    let world_bevy_meshes = meshes_from_gothic_mesh(&world_mesh);

    let mut data = ZenGinWorldData {
        // world_meshes: world_bevy_meshes,
        world_model: world_bevy_meshes,
        ..Default::default()
    };
    // data.world_meshes = world_bevy_meshes;
    // Make sure that placeholder mesh is loaded
    check_if_path_exists(PLARCEHOLDER_MESH, &vfs);

    for obj in world.root_objects() {
        load_meshes(&vfs, &mut data, &obj);
    }
    let way_net = world.way_net();
    load_way_net_points(&way_net, &mut data);
    for npc_spawn in &vm_state.spawn_npcs {
        if let Some(instance) = vm_state.instance_data.get(&npc_spawn.npc) {
            load_npc(instance, npc_spawn, &mut data);
        } else {
            warn_unimplemented!("Not all NPC instances are handled");
        }
    }

    if !world.npcs().is_empty() {
        warn_unimplemented!("Loading NPCs from world");
    }

    if world.spawn_location_count() > 0 {
        warn_unimplemented!("Handling spawn locations");
    }
    data
}

fn load_way_net_points(way_net: &WayNet, data: &mut ZenGinWorldData) {
    let points = way_net.points();
    for point in points {
        let name = point.name().to_lowercase();
        let pos = get_world_pos(point.position());
        warn_unimplemented!("handle way point direction");
        // let rot = point.direction();
        let tr = Transform::from_translation(pos);
        data.way_points.insert(name, tr);
    }
}

fn check_if_path_exists(mesh_path: &str, vfs: &Vfs) -> bool {
    vfs.resolve_path(mesh_path).is_some()
}

fn try_load_mesh(asset_paths: &[String], vfs: &Vfs) -> Option<String> {
    for asset_path in asset_paths {
        if check_if_path_exists(asset_path, vfs) {
            return Some(asset_path.clone());
        }
    }

    return None;
}

fn load_meshes(vfs: &Vfs, data: &mut ZenGinWorldData, obj: &VirtualObject) {
    let visual = obj.visual();
    let visual_name = visual.name();
    let visual_type = visual.get_type();

    if !visual_name.is_empty() && obj.show_visual() {
        assert!(obj.get_type() != VobType::zCVobLight);
        let asset_path = match visual_type {
            VisualType::MULTI_RESOLUTION_MESH => {
                let asset_path = compiled_asset_path(&visual_name, ".3DS", ".MRM");
                if check_if_path_exists(&asset_path, vfs) {
                    Some(asset_path)
                } else {
                    Some(PLARCEHOLDER_MESH.to_string())
                }
            }
            VisualType::MESH => {
                let asset_path = compiled_asset_path(&visual_name, ".3DS", ".MSH");
                if check_if_path_exists(&asset_path, vfs) {
                    Some(asset_path)
                } else {
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
            VisualType::MODEL => {
                let asset_paths: Vec<String> = vec![
                    // Model with hierarchy
                    format!(
                        "/_WORK/DATA/ANIMS/_COMPILED/{}",
                        visual_name.replace(".MDS", ".MDL")
                    ),
                    // Model only
                    format!(
                        "/_WORK/DATA/ANIMS/_COMPILED/{}",
                        visual_name.replace(".MDS", ".MDM")
                    ),
                ];

                if visual_name.ends_with(".ASC") {
                    warn_unimplemented!("load .ASC objects");
                    None
                } else if let Some(asset_path) = try_load_mesh(&asset_paths, vfs) {
                    Some(asset_path)
                } else {
                    warn!("Failed to load visual({})", visual_name);
                    Some(PLARCEHOLDER_MESH.to_string())
                }
            }
            VisualType::MORPH_MESH => {
                let asset_path = format!(
                    "/_WORK/DATA/ANIMS/_COMPILED/{}",
                    visual_name.replace(".MMS", ".MMB")
                );

                if check_if_path_exists(&asset_path, vfs) {
                    Some(asset_path)
                } else {
                    warn!("Failed to load morph mesh({})", visual_name);
                    Some(PLARCEHOLDER_MESH.to_string())
                }
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
                archetype: to_asset_path(&asset_path),
            });
        }
    } else {
        handle_other_vob(obj, data);
    }
    for child in obj.children() {
        load_meshes(vfs, data, &child);
    }
}

fn get_obj_tr(obj: &VirtualObject) -> Transform {
    let pos = get_world_pos(obj.position());
    let rot = get_world_rot(obj.rotation());
    return Transform::from_translation(pos).with_rotation(rot);
}

fn handle_light(obj: &VirtualObject, data: &mut ZenGinWorldData) {
    let pos = get_world_pos(obj.position());
    let rot = get_world_rot(obj.rotation());
    data.light_instances.push(LightInstance { pos, rot });
}
fn handle_other_vob(obj: &VirtualObject, data: &mut ZenGinWorldData) {
    let name = obj.name();
    let type_ = obj.get_type();
    match type_ {
        VobType::zCVob => {
            warn_unimplemented!("VobType::zCVo");
            return;
        }
        VobType::zCVobLevelCompo => {
            warn_unimplemented!("VobType::zCVobLevelComp");
            return;
        }
        VobType::oCItem => {
            warn_unimplemented!("VobType::oCIte");
            return;
        }
        VobType::oCNpc => {
            warn_unimplemented!("VobType::oCNp");
            return;
        }
        VobType::zCMoverController => {
            warn_unimplemented!("VobType::zCMoverControlle");
            return;
        }
        VobType::zCVobScreenFX => {
            warn_unimplemented!("VobType::zCVobScreenF");
            return;
        }
        VobType::zCVobStair => {
            warn_unimplemented!("VobType::zCVobStai");
            return;
        }
        VobType::zCPFXController => {
            warn_unimplemented!("VobType::zCPFXControlle");
            return;
        }
        VobType::zCVobAnimate => {
            warn_unimplemented!("VobType::zCVobAnimat");
            return;
        }
        VobType::zCVobLensFlare => {
            warn_unimplemented!("VobType::zCVobLensFlar");
            return;
        }
        VobType::zCVobLight => {
            handle_light(obj, data);
            return;
        }
        VobType::zCVobSpot => {
            let tr = get_obj_tr(obj);
            data.spots.insert(name.to_lowercase().clone(), tr);
            // warn_unimplemented!("VobType::zCVobSpot");
            return;
        }
        VobType::zCVobStartpoint => {
            warn_unimplemented!("VobType::zCVobStartpoint");
            return;
        }
        VobType::zCMessageFilter => {
            warn_unimplemented!("VobType::zCMessageFilter");
            return;
        }
        VobType::zCCodeMaster => {
            warn_unimplemented!("VobType::zCCodeMaster");
            return;
        }
        VobType::zCTriggerWorldStart => {
            warn_unimplemented!("VobType::zCTriggerWorldStart");
            return;
        }
        VobType::zCCSCamera => {
            warn_unimplemented!("VobType::zCCSCamera");
            return;
        }
        VobType::zCCamTrj_KeyFrame => {
            warn_unimplemented!("VobType::zCCamTrj_KeyFrame");
            return;
        }
        VobType::oCTouchDamage => {
            warn_unimplemented!("VobType::oCTouchDamage");
            return;
        }
        VobType::zCTriggerUntouch => {
            warn_unimplemented!("VobType::zCTriggerUntouch");
            return;
        }
        VobType::zCEarthquake => {
            warn_unimplemented!("VobType::zCEarthquake");
            return;
        }
        VobType::oCMOB => {
            warn_unimplemented!("VobType::oCMOB");
            return;
        }
        VobType::oCMobInter => {
            warn_unimplemented!("VobType::oCMobInter");
            return;
        }
        VobType::oCMobBed => {
            warn_unimplemented!("VobType::oCMobBed");

            return;
        }
        VobType::oCMobFire => {
            warn_unimplemented!("VobType::oCMobFire");
            return;
        }
        VobType::oCMobLadder => {
            warn_unimplemented!("VobType::oCMobLadder");
            return;
        }
        VobType::oCMobSwitch => {
            warn_unimplemented!("VobType::oCMobSwitch");
            return;
        }
        VobType::oCMobWheel => {
            warn_unimplemented!("VobType::oCMobWheel");
            return;
        }
        VobType::oCMobContainer => {
            warn_unimplemented!("VobType::oCMobContainer");
            return;
        }
        VobType::oCMobDoor => {
            warn_unimplemented!("VobType::oCMobDoor");
            return;
        }
        VobType::zCTrigger => {
            warn_unimplemented!("VobType::zCTrigger");
            return;
        }
        VobType::zCTriggerList => {
            warn_unimplemented!("VobType::zCTriggerList");
            return;
        }
        VobType::oCTriggerScript => {
            warn_unimplemented!("VobType::oCTriggerScript");
            return;
        }
        VobType::oCTriggerChangeLevel => {
            warn_unimplemented!("VobType::oCTriggerChangeLevel");
            return;
        }
        VobType::oCCSTrigger => {
            warn_unimplemented!("VobType::oCCSTrigger");
            return;
        }
        VobType::zCMover => {
            warn_unimplemented!("VobType::zCMover");
            return;
        }
        VobType::zCVobSound => {
            warn_unimplemented!("VobType::zCVobSound");
            return;
        }
        VobType::zCVobSoundDaytime => {
            warn_unimplemented!("VobType::zCVobSoundDaytime");
            return;
        }
        VobType::oCZoneMusic => {
            warn_unimplemented!("VobType::oCZoneMusic");
            return;
        }
        VobType::oCZoneMusicDefault => {
            warn_unimplemented!("VobType::oCZoneMusicDefault");
            return;
        }
        VobType::zCZoneZFog => {
            warn_unimplemented!("VobType::zCZoneZFog");
            return;
        }
        VobType::zCZoneZFogDefault => {
            warn_unimplemented!("VobType::zCZoneZFogDefault");
            return;
        }
        VobType::zCZoneVobFarPlane => {
            warn_unimplemented!("VobType::zCZoneVobFarPlane");
            return;
        }
        VobType::zCZoneVobFarPlaneDefault => {
            warn_unimplemented!("VobType::zCZoneVobFarPlaneDefault");
            return;
        }
        VobType::ignored => {
            return;
        }
        VobType::unknown => {
            warn_unimplemented!("VobType::unknown");
            return;
        }
    }
}

fn compiled_asset_path(present_name: &str, replace_from: &str, replace_to: &str) -> String {
    let name = present_name.replace(replace_from, replace_to);
    format!("/_WORK/DATA/MESHES/_COMPILED/{name}")
}
