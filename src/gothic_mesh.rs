use std::{collections::HashMap, ffi::CString};

use bevy::{asset::RenderAssetUsages, mesh::Indices, mesh::PrimitiveTopology, prelude::*};

use ZenKitCAPI_sys::*;

fn get_wolrld_pos(gothic_pos: ZkVec3f) -> Vec3 {
    let pos = ZkVec3f_to_Vec3(gothic_pos) / 100.0;
    // pos.x = -pos.x;
    return pos;
}

#[allow(non_snake_case)]
fn ZkVec3f_to_Vec3(data: ZkVec3f) -> Vec3 {
    Vec3 {
        // x: -data.x,
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

// fn print_nodes(node: *const ZkVfsNode, level: u8) {
//     use std::os::raw::c_void;
//     unsafe {
//         let name = ZkString_to_String(ZkVfsNode_getName(node));
//         for _i in 0..level {
//             print!(" ");
//         }
//         println!("{name}");

//         extern "C" fn callback(ctx: *mut c_void, node: *const ZkVfsNode) -> ZkBool {
//             let level: u8 = ctx as u8;
//             print_nodes(node, level + 1);
//             0
//         }

//         ZkVfsNode_enumerateChildren(node, Some(callback), level as *mut c_void);
//     }
// }

#[derive(Debug, Default)]
struct MeshData {
    indices: Vec<u32>,
    vertices: Vec<Vec3>,
    uvs: Vec<Vec2>,
    normals: Vec<Vec3>,
    colors: Vec<Vec4>,
}

pub fn create_gothic_world_mesh() -> HashMap<String, Mesh> {
    // let mut indices: Vec<u32> = Vec::new();
    // let mut vertices: Vec<Vec3> = Vec::new();
    // let mut uvs: Vec<Vec2> = Vec::new();
    // let mut normals: Vec<Vec3> = Vec::new();
    // let mut colors: Vec<Vec4> = Vec::new();

    let mut meshes: HashMap<String, MeshData> = HashMap::new();

    unsafe {
        let vfs = ZkVfs_new();

        ZkVfs_mountDiskHost(
            vfs,
            c"/media/MM_HDD_DATA/SteamLibrary/steamapps/common/Gothic II/Data/Worlds.vdf".as_ptr(),
            ZkVfsOverwriteBehavior::ZkVfsOverwriteBehavior_ALL,
        );
        ZkVfs_mountDiskHost(
            vfs,
            c"/media/MM_HDD_DATA/SteamLibrary/steamapps/common/Gothic II/Data/Textures.vdf"
                .as_ptr(),
            ZkVfsOverwriteBehavior::ZkVfsOverwriteBehavior_ALL,
        );

        // let node = ZkVfs_getRoot(vfs);
        // print_nodes(node, 0);

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
        // for position_index in 0..positions_count {
        //     // let position = ZkMesh_getPosition(mesh, position_index);
        //     // // println!(" {position_index}) {position:?}");
        //     // vertices.push(get_wolrld_pos(position));
        // }

        // uvs.set_len(vertices.len());
        // normals.set_len(vertices.len());

        let mut used_textures: HashMap<String, u32> = HashMap::new();
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
            let polygon_features =
                std::slice::from_raw_parts(features_ptr, features_count as usize);

            if polygon_indices.len() != 3 {
                // println!(
                //     "Skip polygon({polygon_index}) it has {} vertices and we support only polygons with 3 vertices, features_count({features_count})",
                //     polygon_indices.len()
                // );
                continue;
            }
            assert!(polygon_features.len() == polygon_indices.len());

            // for feature_index in polygon_features {
            //     let feature = ZkMesh_getVertex(mesh, u64::from(*feature_index));
            //     // uvs[] = Vec2::from([feature.texture.x, feature.texture.y]););
            //     // normals.push(Vec3 {
            //     //     x: feature.normal.x,
            //     //     y: feature.normal.y,
            //     //     z: feature.normal.z,
            //     // });
            // }

            // let index0 = 2;
            // let index1 = 1;
            // let index2 = 0;

            let index0 = 0;
            let index1 = 1;
            let index2 = 2;

            let material_index = ZkPolygon_getMaterialIndex(polygon);
            let material = ZkMesh_getMaterial(mesh, u64::from(material_index));
            let material_color = ZkMaterial_getColor(material);
            let texture_path = ZkString_to_String(ZkMaterial_getTexture(material));

            if texture_path.is_empty() {
                // println!("Skip polygon({polygon_index}) it has empty texture");
                continue;
            }

            used_textures
                .entry(texture_path.clone())
                .and_modify(|e| *e += 1)
                .or_default();

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

            let feature0 = ZkMesh_getVertex(mesh, u64::from(polygon_features[index0]));
            let feature1 = ZkMesh_getVertex(mesh, u64::from(polygon_features[index1]));
            let feature2 = ZkMesh_getVertex(mesh, u64::from(polygon_features[index2]));

            uvs.push(ZkVec2f_to_Vec2(feature0.texture));
            uvs.push(ZkVec2f_to_Vec2(feature1.texture));
            uvs.push(ZkVec2f_to_Vec2(feature2.texture));

            // uvs.extend_from_slice(&[
            //     Vec2 { x: 0.0, y: -1.0 },
            //     Vec2 { x: 0.0, y: -0.3 },
            //     Vec2 { x: 0.9, y: -0.3 },
            // ]);

            // uvs.extend_from_slice(&[
            //     Vec2 { x: 0.0, y: 1.0 },
            //     Vec2 { x: 0.0, y: 0.3 },
            //     Vec2 { x: 1.9, y: 0.3 },
            // ]);

            normals.push(ZkVec3f_to_Vec3(feature0.normal));
            normals.push(ZkVec3f_to_Vec3(feature1.normal));
            normals.push(ZkVec3f_to_Vec3(feature2.normal));

            colors.push(material_color);
            colors.push(material_color);
            colors.push(material_color);

            vertices.push(get_wolrld_pos(ZkMesh_getPosition(
                mesh,
                u64::from(polygon_indices[index0]),
            )));
            vertices.push(get_wolrld_pos(ZkMesh_getPosition(
                mesh,
                u64::from(polygon_indices[index1]),
            )));
            vertices.push(get_wolrld_pos(ZkMesh_getPosition(
                mesh,
                u64::from(polygon_indices[index2]),
            )));

            // println!("vertices: {:?}", vertices);
            // println!("uvs: {:?}", uvs);
            // println!("texture: {}", &texture_path);
            // Polygon 378936
            // vertices: [Vec3(4.2808394, -1.8159808, -6.3488526), Vec3(4.733876, -1.7221942, -10.133108), Vec3(-0.82698286, -1.8059807, -10.325498)]
            // uvs: [Vec2(0.09486014, -1.0270786), Vec2(0.019355237, -0.39636958), Vec2(0.9461645, -0.3643043)]
            // texture: NW_CITY_HAFENKAI_BODEN_01.TGA

            // vertices: [Vec3(4.2, -1.8, -6.3), Vec3(4.7, -1.7, -10.1), Vec3(-0.8, -1.8, -10.3)]
            // uvs:      [Vec2(0.0, -1.0), Vec2(0.0, -0.3), Vec2(0.9, -0.3)]

            indices.push(indices.len() as u32);
            indices.push(indices.len() as u32);
            indices.push(indices.len() as u32);
        }
        // println!("used_textures({:?})", used_textures);
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
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, mesh_data.uvs);
        bevy_meshes.insert(texture_str, mesh);
    }

    bevy_meshes

    // .with_computed_area_weighted_normals()
}
