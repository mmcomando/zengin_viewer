use std::{collections::HashMap, ffi::CString};

use bevy::{asset::RenderAssetUsages, mesh::Indices, mesh::PrimitiveTopology, prelude::*};

use ZenKitCAPI_sys::*;

fn get_wolrld_pos(gothic_pos: ZkVec3f) -> Vec3 {
    let pos = ZkVec3f_to_Vec3(gothic_pos) / 100.0;
    return pos;
}

#[allow(non_snake_case)]
fn ZkVec3f_to_Vec3(data: ZkVec3f) -> Vec3 {
    Vec3 {
        x: -data.x,
        y: data.y,
        z: data.z,
    }
}

#[allow(non_snake_case)]
fn ZkVec2f_to_Vec2(data: ZkVec2f) -> Vec2 {
    Vec2 {
        x: data.x,
        y: data.y,
    }
}

#[allow(non_snake_case)]
fn ZkString_to_String(zk_str: ZkString) -> String {
    use std::ffi::CStr;
    let cstr = unsafe { CStr::from_ptr(zk_str) };
    let str = String::from_utf8_lossy(cstr.to_bytes()).to_string();
    return str;
}

fn print_nodes(node: *const ZkVfsNode, level: u8) {
    use std::os::raw::c_void;
    unsafe {
        let name = ZkString_to_String(ZkVfsNode_getName(node));
        for _i in 0..level {
            print!(" ");
        }
        println!("{name}");

        extern "C" fn callback(ctx: *mut c_void, node: *const ZkVfsNode) -> ZkBool {
            let level: u8 = ctx as u8;
            print_nodes(node, level + 1);
            0
        }

        ZkVfsNode_enumerateChildren(node, Some(callback), level as *mut c_void);
    }
}

#[derive(Debug, Default)]
struct MeshData {
    indices: Vec<u32>,
    vertices: Vec<Vec3>,
    uvs: Vec<Vec2>,
    normals: Vec<Vec3>,
    colors: Vec<Vec4>,
}

pub fn create_gothic_world_mesh() -> HashMap<String, Mesh> {
    let mut meshes: HashMap<String, MeshData> = HashMap::new();

    unsafe {
        let vfs = ZkVfs_new();

        ZkVfs_mountDiskHost(
            vfs,
            c"/media/MM_HDD_DATA/SteamLibrary/steamapps/common/Gothic II/Data/Worlds.vdf".as_ptr(),
            ZkVfsOverwriteBehavior::ALL,
        );
        ZkVfs_mountDiskHost(
            vfs,
            c"/media/MM_HDD_DATA/SteamLibrary/steamapps/common/Gothic II/Data/Textures.vdf"
                .as_ptr(),
            ZkVfsOverwriteBehavior::ALL,
        );

        let root_node = ZkVfs_getRoot(vfs);
        print_nodes(root_node, 0);

        let world_node = ZkVfs_resolvePath(
            vfs,
            CString::from(c"/_WORK/DATA/WORLDS/NEWWORLD/NEWWORLD.ZEN").as_ptr(),
        );
        println!("world_node {:?}", world_node);

        let world_read = ZkVfsNode_open(world_node);
        println!("world_read {:?}", world_read);
        let world = ZkWorld_load(world_read);
        println!("world {:?}", world);

        let mesh = ZkWorld_getMesh(world);
        println!("mesh {:?}", mesh);

        let positions_count = ZkMesh_getPositionCount(mesh);
        println!("Positions({positions_count}):");

        let polygons_count = ZkMesh_getPolygonCount(mesh);
        println!("PolygonsCount({polygons_count}):");
        // for polygon_index in 378936..378937 {
        for polygon_index in 0..polygons_count {
            let polygon = ZkMesh_getPolygon(mesh, polygon_index);

            if ZkPolygon_getIsPortal(polygon) == 1 {
                // println!("Skip polygon({polygon_index}) it is portal");
                continue;
            }

            if ZkPolygon_getIsOccluder(polygon) == 1 {
                // println!("polygon({polygon_index}) is Occluder");
                // continue;
            }
            if ZkPolygon_getIsSector(polygon) == 1 {
                // println!("polygon({polygon_index}) is Sector");
                // continue;
            }
            // if ZkPolygon_getShouldRelight(polygon) == 1 {
            //     println!("polygon({polygon_index}) is ShouldRelight");
            // }
            if ZkPolygon_getIsOutdoor(polygon) == 1 {
                println!("polygon({polygon_index}) is Outdoor");
                continue;
            }
            if ZkPolygon_getIsGhostOccluder(polygon) == 1 {
                panic!("polygon({polygon_index}) is GhostOccluder");
            }
            // if ZkPolygon_getIsDynamicallyLit(polygon) == 1 {
            //     println!("polygon({polygon_index}) is DynamicallyLit");
            // }
            if ZkPolygon_getIsLod(polygon) == 1 {
                panic!("polygon({polygon_index}) is Lod");
            }

            let mut indices_count: ZkSize = 0;
            let indices_ptr =
                ZkPolygon_getPositionIndices(polygon, mesh, &mut indices_count as *mut _);
            let polygon_indices = std::slice::from_raw_parts(indices_ptr, indices_count as usize);

            let mut features_count: ZkSize = 0;
            let features_ptr =
                ZkPolygon_getFeatureIndices(polygon, mesh, &mut features_count as *mut _);
            let polygon_features_indices =
                std::slice::from_raw_parts(features_ptr, features_count as usize);

            assert!(polygon_features_indices.len() == polygon_indices.len());

            let material_index = ZkPolygon_getMaterialIndex(polygon);
            let material = ZkMesh_getMaterial(mesh, u64::from(material_index));
            let material_color = ZkMaterial_getColor(material);
            let texture_path = ZkString_to_String(ZkMaterial_getTexture(material));

            if texture_path.is_empty() {
                // println!("Skip polygon({polygon_index}) it has empty texture");
                continue;
            }

            let MeshData {
                uvs,
                normals,
                indices,
                vertices,
                colors,
            } = meshes
                .entry(texture_path.clone())
                .or_insert(MeshData::default());

            let material_color = Vec4::from_array([
                material_color.r as f32 / 255.0,
                material_color.g as f32 / 255.0,
                material_color.b as f32 / 255.0,
                material_color.a as f32 / 255.0,
            ]);

            let triangles_num = polygon_indices.len() - 2;
            let trinagle_indices_num = 3;

            for triangle_index in 0..triangles_num {
                for index in 0..trinagle_indices_num {
                    let idx = if index == 0 {
                        0
                    } else {
                        triangle_index + index
                    };
                    let idx_feature = polygon_features_indices[idx];
                    let feature = ZkMesh_getVertex(mesh, u64::from(idx_feature));
                    uvs.push(ZkVec2f_to_Vec2(feature.texture));
                    normals.push(ZkVec3f_to_Vec3(feature.normal));
                }
            }

            for triangle_index in 0..triangles_num {
                for index in 0..trinagle_indices_num {
                    colors.push(material_color);
                    indices.push(indices.len() as u32);

                    let idx = if index == 0 {
                        0
                    } else {
                        triangle_index + index
                    };
                    vertices.push(get_wolrld_pos(ZkMesh_getPosition(
                        mesh,
                        u64::from(polygon_indices[idx]),
                    )));
                }
            }
        }
    }

    let mut bevy_meshes = HashMap::new();

    for (texture_str, mesh_data) in meshes {
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, mesh_data.colors)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, mesh_data.vertices)
        .with_inserted_indices(Indices::U32(mesh_data.indices))
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, mesh_data.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, mesh_data.uvs)
        .with_generated_tangents()
        .unwrap();
        bevy_meshes.insert(texture_str, mesh);
    }

    bevy_meshes
}
