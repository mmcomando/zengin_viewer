use std::collections::HashMap;

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
        mesh::meshes_from_gothic_mesh,
        mesh_model::{meshes_from_gothic_model, meshes_from_gothic_model_mesh},
        mesh_morph::meshes_from_gothic_morph_mesh,
        mesh_mrs::meshes_from_gothic_mrs_mesh,
        script_vm::{InstanceState, SpawnNpc},
    },
};

const PLARCEHOLDER_MESH: &str = "/_WORK/DATA/MESHES/_COMPILED/SPHERE.MRM";
const HUMAN_MODEL: &str = "/_WORK/DATA/ANIMS/_COMPILED/HUM_BODY_NAKED0.MDM";
const HUMAN_MODEL_HIERARCHY: &str = "/_WORK/DATA/ANIMS/_COMPILED/HUMANS_RELAXED.MDH";

pub struct ZenKitWorldData {
    pub meshes: HashMap<String, Vec<LoadedMeshData>>,
    pub mesh_instances: Vec<MeshInstance>,
    pub light_instances: Vec<LightInstance>,
    pub spots: HashMap<String, Transform>,
    pub way_points: HashMap<String, Transform>,
    pub index: u32,
}

pub fn load_npc(
    instance: &InstanceState,
    spawn_npc: &SpawnNpc,
    vfs: &Vfs,
    data: &mut ZenKitWorldData,
) {
    let way_point_name = spawn_npc
        .routine_way_point_name
        .as_ref()
        .unwrap_or(&spawn_npc.way_point_name);
    let Some(tr) = data.way_points.get(&way_point_name.to_lowercase()) else {
        println!("way_point({}) not found", way_point_name.to_lowercase());
        return;
    };

    let cols = 16;
    let ind_y = data.index / cols;
    let ind_x = data.index % cols;
    // data.index += 1;

    let tr = Transform::from_translation(Vec3 {
        x: 1.0 + (ind_x as f32 * 1.0),
        y: 0.0 + (ind_y as f32 * 2.0),
        z: 0.0,
    }) * *tr;

    let body_dt = Vec3 {
        x: -0.01,
        y: -0.5,
        z: -0.08,
    };

    let tr_body = tr * Transform::from_translation(body_dt);
    load_mesh(HUMAN_MODEL, &vfs, &mut data.meshes);
    data.mesh_instances.push(MeshInstance {
        mesh_path: HUMAN_MODEL.to_string(),
        pos: tr_body.translation,
        rot: tr_body.rotation,
        is_colider: false,
        texture_override: Some(instance.body_texture.to_string()),
    });
    let head_dt = Vec3 {
        x: 0.0,
        y: 0.5,
        z: 0.0,
    };
    let head_transform = tr
        * Transform::from_translation(head_dt)
        * data.meshes.get(HUMAN_MODEL).unwrap()[0]
            .head_transform
            .unwrap();

    let model = format!(
        "/_WORK/DATA/ANIMS/_COMPILED/{}.MMB",
        instance.head_model.to_uppercase().to_string()
    );
    load_mesh(&model, &vfs, &mut data.meshes);
    data.mesh_instances.push(MeshInstance {
        mesh_path: model.to_string(),
        pos: head_transform.translation,
        rot: head_transform.rotation,
        is_colider: false,
        texture_override: Some(instance.face_texture.to_string()),
    });
}

pub fn create_gothic_world_mesh(
    world_path: &str,
    vm_state: &crate::zengin::script_vm::State,
) -> ZenKitWorldData {
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

    let mut bevy_meshes = HashMap::new();
    let mut object_instances = Vec::new();

    let world_mesh = world.mesh();
    let world_bevy_meshes = meshes_from_gothic_mesh(&world_mesh);

    bevy_meshes.insert(world_path.to_string(), world_bevy_meshes);
    object_instances.push(MeshInstance {
        mesh_path: world_path.to_string(),
        pos: Vec3::ZERO,
        rot: Quat::IDENTITY,
        is_colider: true,
        texture_override: None,
    });

    // Make sure that placeholder mesh is loaded
    load_mesh(PLARCEHOLDER_MESH, &vfs, &mut bevy_meshes);

    let mut data = ZenKitWorldData {
        meshes: bevy_meshes,
        mesh_instances: object_instances,
        light_instances: vec![],
        spots: HashMap::new(),
        way_points: HashMap::new(),
        index: 0,
    };
    for obj in world.root_objects() {
        load_meshes(&vfs, &mut data, &obj);
    }
    let way_net = world.way_net();
    loat_way_net_points(&way_net, &mut data);
    for npc_spawn in &vm_state.spawn_npcs {
        if let Some(instance) = vm_state.instance_state.get(&npc_spawn.npc_name) {
            load_npc(instance, npc_spawn, &vfs, &mut data);
        } else {
            println!("There no instance for npc({})", npc_spawn.npc_name);
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

fn loat_way_net_points(way_net: &WayNet, data: &mut ZenKitWorldData) {
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

fn load_mesh(
    mesh_path: &str,
    vfs: &Vfs,
    bevy_meshes: &mut HashMap<String, Vec<LoadedMeshData>>,
) -> bool {
    if bevy_meshes.contains_key(mesh_path) {
        // println!("Already loaded mesh_path({mesh_path})");
        return true;
    }
    let Some(node) = vfs.resolve_path(mesh_path) else {
        // println!("Mesh({mesh_path}) not found");
        return false;
    };

    let Some(read) = node.open() else {
        println!("Failed to open mesh({mesh_path})");
        return false;
    };

    let meshes = if mesh_path.ends_with(".MRM") {
        let mesh = zen_kit_rs::mrs_mesh::MrsMesh::load(&read).unwrap();
        meshes_from_gothic_mrs_mesh(&mesh)
    } else if mesh_path.ends_with(".MSH") {
        let mesh = zen_kit_rs::mesh::Mesh::load(&read).unwrap();
        meshes_from_gothic_mesh(&mesh)
    } else if mesh_path.ends_with(".MDL") {
        let mesh = zen_kit_rs::model::Model::load(&read).unwrap();
        meshes_from_gothic_model(&mesh)
    } else if mesh_path.ends_with(".MMB") {
        let mesh = zen_kit_rs::morph_mesh::MorphMesh::load(&read).unwrap();
        meshes_from_gothic_morph_mesh(&mesh)
    } else if mesh_path.ends_with(".MDM") {
        let mesh = zen_kit_rs::model::ModelMesh::load(&read).unwrap();
        // We try to load model only, but maybe there is coresponding hierarchy file
        // If we have hierarchy file load it and use it
        let mut hierarchy_path = mesh_path.replace("MDM", "MDH");
        if mesh_path == HUMAN_MODEL {
            hierarchy_path = HUMAN_MODEL_HIERARCHY.to_string();
        }

        let model_hierarchy = if let Some(hierarchy_node) = vfs.resolve_path(&hierarchy_path) {
            if let Some(read_hierarchy) = hierarchy_node.open() {
                zen_kit_rs::model::Model::load(&read_hierarchy)
            } else {
                None
            }
        } else {
            None
        };
        meshes_from_gothic_model_mesh(&mesh, model_hierarchy.as_ref())
    } else if mesh_path.ends_with(".MSB") {
        let mesh = zen_kit_rs::model::Model::load(&read).unwrap();
        meshes_from_gothic_model(&mesh)
    } else if mesh_path.ends_with(".MDH") {
        let mesh = zen_kit_rs::model::Model::load(&read).unwrap();
        meshes_from_gothic_model(&mesh)
    } else {
        println!("mesh_path({}) unrecognized mesh format", mesh_path);
        return false;
    };

    if meshes.is_empty() {
        println!("mesh_path({}) doesn't contain any meshes", mesh_path);
        return false;
    }
    // info!("Load mesh_path({})", mesh_path,);
    bevy_meshes.insert(mesh_path.to_string(), meshes);
    true
}

fn try_load_mesh(
    asset_paths: &[String],
    vfs: &Vfs,
    bevy_meshes: &mut HashMap<String, Vec<LoadedMeshData>>,
) -> Option<String> {
    for asset_path in asset_paths {
        if load_mesh(asset_path, vfs, bevy_meshes) {
            return Some(asset_path.to_string());
        }
    }

    return None;
}

fn load_meshes(vfs: &Vfs, data: &mut ZenKitWorldData, obj: &VirtualObject) {
    let visual = obj.visual();
    let visual_name = visual.name();
    let visual_type = visual.get_type();
    let pos = get_world_pos(obj.position());
    let rot_quat = get_world_rot(obj.rotation());

    if !visual_name.is_empty() && obj.show_visual() {
        assert!(obj.get_type() != VobType::zCVobLight);
        let asset_path = match visual_type {
            VisualType::MULTI_RESOLUTION_MESH => {
                let asset_path = compiled_asset_path(&visual_name, ".3DS", ".MRM");
                if load_mesh(&asset_path, vfs, &mut data.meshes) {
                    Some(asset_path)
                } else {
                    Some(PLARCEHOLDER_MESH.to_string())
                }
            }
            VisualType::MESH => {
                let asset_path = compiled_asset_path(&visual_name, ".3DS", ".MSH");
                if load_mesh(&asset_path, vfs, &mut data.meshes) {
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
                } else if let Some(asset_path) = try_load_mesh(&asset_paths, vfs, &mut data.meshes)
                {
                    // warn!("load visual({})", visual_name);
                    // None
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

                if load_mesh(&asset_path, vfs, &mut data.meshes) {
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
            data.mesh_instances.push(MeshInstance {
                mesh_path: asset_path,
                pos,
                rot: rot_quat,
                is_colider: false,
                texture_override: None,
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

fn handle_light(obj: &VirtualObject, data: &mut ZenKitWorldData) {
    let pos = get_world_pos(obj.position());
    let rot = get_world_rot(obj.rotation());
    data.light_instances.push(LightInstance { pos, rot })
}
fn handle_other_vob(obj: &VirtualObject, data: &mut ZenKitWorldData) {
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
