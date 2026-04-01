use std::fmt::Debug;

use zen_kit_rs::{stream::Read, texture::Texture};

use bevy::{
    asset::{AssetLoader, LoadContext, RenderAssetUsages, io::Reader},
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    reflect::TypePath,
    render::render_resource::{Extent3d, TextureDataOrder, TextureDimension, TextureFormat},
};

#[derive(Default, TypePath)]
pub struct ZenGinTextureLoader;

impl AssetLoader for ZenGinTextureLoader {
    type Asset = Image;
    type Settings = ();
    type Error = BevyError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let zengin_data_with_mips = get_zengin_texture_data(&bytes);
        let zengin_data = zengin_data_with_mips.mips[0].clone();
        assert!(
            zengin_data.size.width * zengin_data.size.height * 4
                == zengin_data.data_rgba.len() as u32
        );
        if zengin_data.size.width == 0 || zengin_data.size.height == 0 {
            return Err(BevyError::from("Texture has 0 dimensions"));
        }

        let mut image_data = Vec::new();
        image_data.reserve_exact(
            zengin_data_with_mips
                .mips
                .iter()
                .map(|mip| mip.data_rgba.len())
                .sum(),
        );
        zengin_data_with_mips
            .mips
            .iter()
            .for_each(|mip| image_data.extend(&mip.data_rgba));

        let mut image = Image::default();
        image.texture_descriptor.format = TextureFormat::Rgba8Unorm;
        image.data = Some(image_data);
        image.data_order = TextureDataOrder::MipMajor;
        image.texture_descriptor.size = zengin_data_with_mips.mips[0].size;
        image.texture_descriptor.mip_level_count = zengin_data_with_mips.mips.len() as u32;
        image.texture_descriptor.dimension = TextureDimension::D2;

        image.asset_usage = RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD;

        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            address_mode_w: ImageAddressMode::Repeat,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Linear,
            ..ImageSamplerDescriptor::default()
        });

        Ok(image)
    }

    fn extensions(&self) -> &[&str] {
        &["TEX"]
    }
}

#[derive(Debug, Clone)]
pub struct ZenGinTexture {
    data_rgba: Vec<u8>,
    size: Extent3d,
}

#[derive(Debug, Clone)]
pub struct ZenGinTextureWithMips {
    mips: Vec<ZenGinTexture>,
}

pub fn get_zengin_texture_data(bytes: &[u8]) -> ZenGinTextureWithMips {
    let texture_read = Read::from_slice(bytes).unwrap();
    let texture = Texture::load(&texture_read).unwrap();

    let mips_count = texture.mipmap_count();

    let mut mips: Vec<ZenGinTexture> = vec![];
    for mip_index in 0..mips_count {
        let width = texture.width_mipmap(u64::from(mip_index));
        let height = texture.height_mipmap(u64::from(mip_index));

        let size = (4 * width * height) as usize;
        let data = texture.mipmap_rgba(u64::from(mip_index));
        assert!(size == data.len());

        let zengin_texture = ZenGinTexture {
            data_rgba: data,
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        };
        mips.push(zengin_texture);
    }

    ZenGinTextureWithMips { mips }
}
