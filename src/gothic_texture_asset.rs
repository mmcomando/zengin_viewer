use std::fmt::Debug;

use ZenKitCAPI_sys::*;

use bevy::{
    asset::{AssetLoader, LoadContext, RenderAssetUsages, io::Reader},
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    reflect::TypePath,
    render::render_resource::{Extent3d, TextureDataOrder, TextureDimension, TextureFormat},
};

#[derive(Default, TypePath)]
pub struct GothicTextureLoader;

impl AssetLoader for GothicTextureLoader {
    type Asset = Image;
    type Settings = ();
    type Error = BevyError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        // info!("Loading Gothic Texture...");
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        // info!("Loading Gothic Texture bytes({})", bytes.len());

        let gothic_data_with_mips = get_gothic_texture_data(&bytes);
        let gothic_data = gothic_data_with_mips.mips[0].clone();
        // println!("gothic_data({:?})", gothic_data);
        assert!(
            gothic_data.size.width * gothic_data.size.height * 4
                == gothic_data.data_rgba.len() as u32
        );
        if gothic_data.size.width == 0 || gothic_data.size.height == 0 {
            return Err(BevyError::from("Texture has 0 dimensions"));
        }

        // let mut image: Image = Image::new(
        //     gothic_data.size,
        //     TextureDimension::D2,
        //     gothic_data.data_rgba,
        //     TextureFormat::Rgba8Unorm,
        //     RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        // );
        // let mut image: Image = Image::from_buffer(buffer, image_type, supported_compressed_formats, is_srgb, image_sampler, asset_usage)

        let mut image_data = Vec::new();
        image_data.reserve_exact(
            gothic_data_with_mips
                .mips
                .iter()
                .map(|mip| mip.data_rgba.len())
                .sum(),
        );
        gothic_data_with_mips
            .mips
            .iter()
            .for_each(|mip| image_data.extend(&mip.data_rgba));

        let mut image = Image::default();
        image.texture_descriptor.format = TextureFormat::Rgba8Unorm;
        image.data = Some(image_data);
        image.data_order = TextureDataOrder::MipMajor;
        image.texture_descriptor.size = gothic_data_with_mips.mips[0].size;
        image.texture_descriptor.mip_level_count = gothic_data_with_mips.mips.len() as u32;
        image.texture_descriptor.dimension = TextureDimension::D2;

        image.asset_usage = RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD;

        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            // label: (),
            // address_mode_u: ImageAddressMode::MirrorRepeat,
            // address_mode_v: ImageAddressMode::MirrorRepeat,
            // address_mode_w: ImageAddressMode::MirrorRepeat,
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            address_mode_w: ImageAddressMode::Repeat,
            // mag_filter: (),
            // min_filter: (),
            // mipmap_filter: (),
            // lod_min_clamp: (),
            // lod_max_clamp: (),
            // compare: (),
            // anisotropy_clamp: (),
            // border_color: (),
            ..ImageSamplerDescriptor::default()
        });

        Ok(image)
    }

    fn extensions(&self) -> &[&str] {
        &["TEX"]
    }
}

#[derive(Debug, Clone)]
pub struct GothicTexture {
    data_rgba: Vec<u8>,
    size: Extent3d,
}

#[derive(Debug, Clone)]
pub struct GothicTextureWithMips {
    mips: Vec<GothicTexture>,
}

pub fn get_gothic_texture_data(bytes: &[u8]) -> GothicTextureWithMips {
    unsafe {
        let texture_read = ZkRead_newMem(bytes.as_ptr(), bytes.len() as ZkSize);
        let texture = ZkTexture_load(texture_read);

        let mips_count = ZkTexture_getMipmapCount(texture);

        let mut mips: Vec<GothicTexture> = vec![];
        for mip_index in 0..mips_count {
            let width = ZkTexture_getWidthMipmap(texture, u64::from(mip_index));
            let height = ZkTexture_getHeightMipmap(texture, u64::from(mip_index));

            let size = (4 * width * height) as usize;
            let mut data = Vec::with_capacity(size);
            data.set_len(size);
            let written_bytes = ZkTexture_getMipmapRgba(
                texture,
                u64::from(mip_index),
                data.as_mut_ptr(),
                data.len() as ZkSize,
            );
            assert!(written_bytes as usize == data.len());

            let gothic_texture = GothicTexture {
                data_rgba: data,
                size: Extent3d {
                    width: width,
                    height: height,
                    depth_or_array_layers: 1,
                },
            };
            // println!("loaded gothic_texture({:?})", gothic_texture.size);
            mips.push(gothic_texture);
        }

        return GothicTextureWithMips { mips };
    }
}
