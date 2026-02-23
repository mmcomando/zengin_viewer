use std::{collections::HashMap, ffi::CString};

use bevy::{asset::RenderAssetUsages, mesh::Indices, mesh::PrimitiveTopology, prelude::*};

fn get_wolrld_pos(gothic_pos: ZenKitCAPI_sys::ZkVec3f) -> Vec3 {
    let pos = ZkVec3f_to_Vec3(gothic_pos) / 100.0;
    // pos.x = -pos.x;
    return pos;
}

#[allow(non_snake_case)]
fn ZkVec3f_to_Vec3(data: ZenKitCAPI_sys::ZkVec3f) -> Vec3 {
    Vec3 {
        x: -data.x,
        y: data.y,
        z: data.z,
    }
}

#[allow(non_snake_case)]
fn ZkVec2f_to_Vec2(data: ZenKitCAPI_sys::ZkVec2f) -> Vec2 {
    Vec2 {
        x: data.x,
        y: data.y,
    }
}

#[allow(non_snake_case)]
fn ZkString_to_String(zk_str: ZenKitCAPI_sys::ZkString) -> String {
    use std::ffi::CStr;
    let cstr = unsafe { CStr::from_ptr(zk_str) };
    let str = String::from_utf8_lossy(cstr.to_bytes()).to_string();
    return str;
}

fn print_nodes(node: *const ZenKitCAPI_sys::ZkVfsNode, level: u8) {
    use std::os::raw::c_void;
    unsafe {
        let name = ZkString_to_String(ZenKitCAPI_sys::ZkVfsNode_getName(node));
        for _i in 0..level {
            print!(" ");
        }
        println!("{name}");

        extern "C" fn callback(
            ctx: *mut c_void,
            node: *const ZenKitCAPI_sys::ZkVfsNode,
        ) -> ZenKitCAPI_sys::ZkBool {
            let level: u8 = ctx as u8;
            print_nodes(node, level + 1);
            0
        }

        ZenKitCAPI_sys::ZkVfsNode_enumerateChildren(node, Some(callback), level as *mut c_void);
    }
}

pub fn create_gothic_world_mesh() -> Mesh {
    let mut indices: Vec<u32> = Vec::new();
    let mut vertices: Vec<Vec3> = Vec::new();
    let mut uvs: Vec<Vec2> = Vec::new();
    let mut normals: Vec<Vec3> = Vec::new();
    let mut colors: Vec<Vec4> = Vec::new();

    unsafe {
        let vfs = ZenKitCAPI_sys::ZkVfs_new();
        let file = CString::from(
            c"/media/MM_HDD_DATA/SteamLibrary/steamapps/common/Gothic II/Data/Worlds.vdf",
        );
        ZenKitCAPI_sys::ZkVfs_mountDiskHost(
            vfs,
            file.as_ptr(),
            ZenKitCAPI_sys::ZkVfsOverwriteBehavior::ZkVfsOverwriteBehavior_ALL,
        );

        let node = ZenKitCAPI_sys::ZkVfs_getRoot(vfs);
        print_nodes(node, 0);

        let world_node = ZenKitCAPI_sys::ZkVfs_resolvePath(
            vfs,
            CString::from(c"/_WORK/DATA/WORLDS/NEWWORLD/NEWWORLD.ZEN").as_ptr(),
        );
        println!("world_node {:?}", world_node);

        let world_read = ZenKitCAPI_sys::ZkVfsNode_open(world_node);
        println!("world_read {:?}", world_read);
        let world = ZenKitCAPI_sys::ZkWorld_load(world_read);
        println!("world {:?}", world);

        let mesh = ZenKitCAPI_sys::ZkWorld_getMesh(world);
        println!("mesh {:?}", mesh);

        let positions_count = ZenKitCAPI_sys::ZkMesh_getPositionCount(mesh);
        println!("Positions({positions_count}):");
        // for position_index in 0..positions_count {
        //     // let position = ZenKitCAPI_sys::ZkMesh_getPosition(mesh, position_index);
        //     // // println!(" {position_index}) {position:?}");
        //     // vertices.push(get_wolrld_pos(position));
        // }

        // uvs.set_len(vertices.len());
        // normals.set_len(vertices.len());

        let mut used_textures: HashMap<String, u32> = HashMap::new();
        let polygons_count = ZenKitCAPI_sys::ZkMesh_getPolygonCount(mesh);
        println!("Polygons({polygons_count}):");
        for polygon_index in 0..polygons_count {
            let polygon = ZenKitCAPI_sys::ZkMesh_getPolygon(mesh, polygon_index);

            if ZenKitCAPI_sys::ZkPolygon_getIsPortal(polygon) == 1 {
                println!("Skip polygon({polygon_index}) it is portal");
                continue;
            }

            let mut indices_count: ZenKitCAPI_sys::ZkSize = 0;
            let indices_ptr = ZenKitCAPI_sys::ZkPolygon_getPositionIndices(
                polygon,
                mesh,
                &mut indices_count as *mut _,
            );
            let polygon_indices = std::slice::from_raw_parts(indices_ptr, indices_count as usize);

            let mut features_count: ZenKitCAPI_sys::ZkSize = 0;
            let features_ptr = ZenKitCAPI_sys::ZkPolygon_getFeatureIndices(
                polygon,
                mesh,
                &mut features_count as *mut _,
            );
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
            //     let feature = ZenKitCAPI_sys::ZkMesh_getVertex(mesh, u64::from(*feature_index));
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

            let material_index = ZenKitCAPI_sys::ZkPolygon_getMaterialIndex(polygon);
            let material = ZenKitCAPI_sys::ZkMesh_getMaterial(mesh, u64::from(material_index));
            let material_color = ZenKitCAPI_sys::ZkMaterial_getColor(material);
            let texture_path = ZkString_to_String(ZenKitCAPI_sys::ZkMaterial_getTexture(material));
            used_textures
                .entry(texture_path.clone())
                .and_modify(|e| *e += 1)
                .or_default();
            let material_color = Vec4::from_array([
                material_color.r as f32 / 255.0,
                material_color.g as f32 / 255.0,
                material_color.b as f32 / 255.0,
                material_color.a as f32 / 255.0,
            ]);

            let feature0 =
                ZenKitCAPI_sys::ZkMesh_getVertex(mesh, u64::from(polygon_features[index0]));
            let feature1 =
                ZenKitCAPI_sys::ZkMesh_getVertex(mesh, u64::from(polygon_features[index1]));
            let feature2 =
                ZenKitCAPI_sys::ZkMesh_getVertex(mesh, u64::from(polygon_features[index2]));

            uvs.push(ZkVec2f_to_Vec2(feature0.texture));
            uvs.push(ZkVec2f_to_Vec2(feature1.texture));
            uvs.push(ZkVec2f_to_Vec2(feature2.texture));

            normals.push(ZkVec3f_to_Vec3(feature0.normal));
            normals.push(ZkVec3f_to_Vec3(feature1.normal));
            normals.push(ZkVec3f_to_Vec3(feature2.normal));

            colors.push(material_color);
            colors.push(material_color);
            colors.push(material_color);

            vertices.push(get_wolrld_pos(ZenKitCAPI_sys::ZkMesh_getPosition(
                mesh,
                u64::from(polygon_indices[index0]),
            )));
            vertices.push(get_wolrld_pos(ZenKitCAPI_sys::ZkMesh_getPosition(
                mesh,
                u64::from(polygon_indices[index1]),
            )));
            vertices.push(get_wolrld_pos(ZenKitCAPI_sys::ZkMesh_getPosition(
                mesh,
                u64::from(polygon_indices[index2]),
            )));

            indices.push(indices.len() as u32);
            indices.push(indices.len() as u32);
            indices.push(indices.len() as u32);
        }
        // println!("used_textures({:?})", used_textures);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
    .with_inserted_indices(Indices::U32(indices))
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    // .with_computed_area_weighted_normals()
}
