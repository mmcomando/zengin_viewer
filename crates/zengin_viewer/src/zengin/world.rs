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
        conversions::*,
        script::state::{InstanceState, SpawnItem, SpawnNpc},
        visual::mesh::meshes_from_zengin_mesh,
        waypoint_graph::{WaypointGraph, WaypointNode},
    },
};

const PLARCEHOLDER_MESH: &str = "/_WORK/DATA/MESHES/_COMPILED/SPHERE.MRM";

fn normalize_mesh_name(name: &str) -> String {
    name.to_uppercase()
        .replace(".3DS", "")
        .replace(".MMS", "")
        .replace(".MDS", "")
        .replace(".ASC", "")
}

fn find_mesh_path(vfs: &Vfs, name: &str) -> Option<String> {
    let name = normalize_mesh_name(name);

    let asset_paths: Vec<String> = vec![
        format!("/_WORK/DATA/ANIMS/_COMPILED/{}.MMB", name),
        format!("/_WORK/DATA/ANIMS/_COMPILED/{}.MDL", name),
        format!("/_WORK/DATA/ANIMS/_COMPILED/{}.MDM", name),
        format!("/_WORK/DATA/ANIMS/_COMPILED/{}.MDH", name),
        format!("/_WORK/DATA/MESHES/_COMPILED/{}.MDB", name),
        format!("/_WORK/DATA/MESHES/_COMPILED/{}.MRM", name),
        format!("/_WORK/DATA/MESHES/_COMPILED/{}.MSH", name),
    ];

    let Some(asset_path) = try_load_mesh(vfs, &asset_paths) else {
        warn!("Some meshes are not found, example({})", name);
        return None;
    };

    let asset_path = to_asset_path(&asset_path);
    return Some(asset_path);
}

/// Standing body state animation of an animation scheme. `S_RUN` is the human
/// idle, most creatures only provide `S_FISTRUN`; probe the anims archive for
/// the one the scheme has.
fn resolve_stand_animation(vfs: &Vfs, scheme: &str) -> Option<String> {
    ["S_RUN", "S_FISTRUN"]
        .into_iter()
        .find(|ani| {
            vfs.resolve_path(&format!("/_WORK/DATA/ANIMS/_COMPILED/{scheme}-{ani}.MAN"))
                .is_some()
        })
        .map(str::to_string)
}

fn get_tr_from_point_name(data: &ZenGinWorldData, name: &str) -> Option<Transform> {
    let tr = if let Some(tr) = data.spots.get(name) {
        tr
    } else if let Some(tr) = data.way_points.get(name) {
        tr
    } else {
        warn_once!("some points are not found, example({})", name);
        return None;
    };
    Some(*tr)
}

pub fn get_item_mesh_path(
    vfs: &Vfs,
    vm_state: &crate::zengin::script::state::LoadedState,
    name: &str,
) -> Option<String> {
    let Some(item_instance) = vm_state.item_instances.get(name) else {
        println!("Failed to find item({}) instance", name);
        return None;
    };
    let visual = item_instance
        .visual_change
        .as_ref()
        .unwrap_or(&item_instance.visual);
    find_mesh_path(vfs, visual)
}
pub fn load_weapon(
    data: &mut ZenGinWorldData,
    vfs: &Vfs,
    vm_state: &crate::zengin::script::state::LoadedState,
    item: &SpawnItem,
) {
    let Some(tr) = get_tr_from_point_name(data, &item.way_point) else {
        return;
    };

    let Some(asset_path) = get_item_mesh_path(vfs, vm_state, &item.instance_name) else {
        println!("not found mesh for item({})", &item.instance_name);
        return;
    };
    let item = ZenGinItem {
        tr,
        model: asset_path,
    };
    data.items.push(item);
}

pub fn load_npc(
    data: &mut ZenGinWorldData,
    vfs: &Vfs,
    vm_state: &crate::zengin::script::state::LoadedState,
    instance: &InstanceState,
    spawn_npc: &SpawnNpc,
    start_hour: u32,
) {
    let way_point_name = instance
        .get_routine_entry(start_hour * 60)
        .map_or(&spawn_npc.way_point, |el| &el.way_point);

    let Some(tr) = get_tr_from_point_name(data, way_point_name) else {
        return;
    };

    let tr = tr.to_matrix();
    let tr = Transform::from_translation(Vec3 {
        x: 0.0,
        y: -0.5,
        z: 0.0,
    })
    .to_matrix()
        * tr;

    let tr_body = tr;

    let Some(body_model) = find_mesh_path(vfs, &instance.body_model) else {
        println!("not found mesh for body model({})", instance.body_model);
        return;
    };
    let head_model = if let Some(head_model) = &instance.head_model {
        if let Some(head_model) = find_mesh_path(vfs, head_model) {
            Some(head_model)
        } else {
            println!("not found mesh for head model({})", head_model);
            None
        }
    } else {
        None
    };

    let armor_model = if let Some(armor_model) = &instance.armor_model {
        if let Some(armor_model) = get_item_mesh_path(vfs, vm_state, armor_model) {
            Some(armor_model)
        } else {
            println!("not found mesh for armor_model({})", armor_model);
            None
        }
    } else {
        None
    };
    let hierarchy = if let Some(hierarchy) = &instance.hierarchy {
        if let Some(hierarchy) = find_mesh_path(vfs, hierarchy) {
            Some(hierarchy)
        } else {
            println!("not found mesh for model({})", hierarchy);
            None
        }
    } else {
        None
    };

    // Animations of an npc live in the scheme of its visual (`.mds`), the body
    // mesh name is unrelated, e.g. the wolf visual `Wolf.mds` uses `WOLF-*.MAN`
    // while its body mesh is `Wol_Body`.
    let animation_scheme = instance.hierarchy.as_deref().map(normalize_mesh_name);
    let stand_animation = animation_scheme
        .as_deref()
        .and_then(|scheme| resolve_stand_animation(vfs, scheme));

    let mut body_tr = Transform::from_matrix(tr_body);
    if let Some(scale) = instance.model_scale {
        body_tr.scale = scale;
    }

    let npc = ZenGinNpc {
        npc_intance_id: spawn_npc.npc_index,
        hierarchy,
        animation_scheme,
        stand_animation,
        model_fatness: instance.model_fatness,
        head_model,
        head_texture: instance
            .face_texture
            .as_ref()
            .map(|el| get_full_texture_path(el)),
        body_tr,
        body_model,
        body_texture: instance
            .body_texture
            .as_ref()
            .map(|el| get_full_texture_path(el)),
        armor_model,
    };
    // println!("Create ZenGinNpc({:?})", npc.body_model);
    data.npcs.push(npc);
}

pub fn load_zengin_world_data(
    world_path: &str,
    vm_state: &crate::zengin::script::state::LoadedState,
    start_hour: u32,
) -> ZenGinWorldData {
    let vfs = Vfs::new();

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

    for obj in world.root_objects() {
        load_objects(&vfs, &mut data, vm_state, &obj);
    }
    let way_net = world.way_net();
    load_way_net_points(&way_net, &mut data);
    for npc_spawn in &vm_state.spawn_npcs {
        // if npc_spawn.npc_index.value() != 10347 {
        //     continue;
        // }
        if let Some(instance) = vm_state.instance_data.get(&npc_spawn.npc_index) {
            load_npc(&mut data, &vfs, vm_state, instance, npc_spawn, start_hour);
        } else {
            println!("unknown data for npc({:?})", npc_spawn);
            // panic!();
        }
    }

    for weapon in &vm_state.spawn_items {
        load_weapon(&mut data, &vfs, vm_state, weapon);
    }

    if !world.npcs().is_empty() {
        warn_unimplemented!("Loading NPCs from world");
    }

    if world.spawn_location_count() > 0 {
        warn_unimplemented!("Handling spawn locations");
    }

    // Prebuild item symbol id -> mesh path for runtime equip
    for (id, visual) in &vm_state.item_visuals {
        if let Some(path) = resolve_mesh_path_quiet(&vfs, visual) {
            data.item_mesh_paths.insert(id.value(), path);
        }
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
            name,
            position: pos,
            direction: Vec3::new(direction.x, direction.y, direction.z),
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

fn try_load_mesh(vfs: &Vfs, asset_paths: &[String]) -> Option<String> {
    for asset_path in asset_paths {
        if vfs.resolve_path(asset_path).is_some() {
            return Some(asset_path.clone());
        }
    }

    return None;
}

/// Like `find_mesh_path` but returns `None` silently, for the bulk item mesh
/// table where missing meshes are expected.
fn resolve_mesh_path_quiet(vfs: &Vfs, name: &str) -> Option<String> {
    let name = normalize_mesh_name(name);
    let asset_paths: Vec<String> = vec![
        format!("/_WORK/DATA/ANIMS/_COMPILED/{}.MMB", name),
        format!("/_WORK/DATA/ANIMS/_COMPILED/{}.MDL", name),
        format!("/_WORK/DATA/ANIMS/_COMPILED/{}.MDM", name),
        format!("/_WORK/DATA/ANIMS/_COMPILED/{}.MDH", name),
        format!("/_WORK/DATA/MESHES/_COMPILED/{}.MDB", name),
        format!("/_WORK/DATA/MESHES/_COMPILED/{}.MRM", name),
        format!("/_WORK/DATA/MESHES/_COMPILED/{}.MSH", name),
    ];
    try_load_mesh(vfs, &asset_paths).map(|path| to_asset_path(&path))
}

fn load_objects(
    vfs: &Vfs,
    data: &mut ZenGinWorldData,
    vm_state: &crate::zengin::script::state::LoadedState,
    obj: &VirtualObject,
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
            });
        }
    } else {
        handle_other_vob(data, vfs, vm_state, obj);
    }
    for child in obj.children() {
        load_objects(vfs, data, vm_state, &child);
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
    vfs: &Vfs,
    vm_state: &crate::zengin::script::state::LoadedState,
    obj: &VirtualObject,
) {
    let name = obj.name();
    let type_ = obj.get_type();
    match type_ {
        VobType::oCItem => {
            let tr = get_obj_tr(obj);
            let name = name.to_lowercase();

            let Some(asset_path) = get_item_mesh_path(vfs, vm_state, &name) else {
                warn!("Not found mesh for item({}) vob", name);
                return;
            };
            let npc = ZenGinItem {
                tr,
                model: asset_path,
            };
            data.items.push(npc);

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
