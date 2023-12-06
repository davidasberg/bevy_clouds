use std::io::BufReader;

use bevy::{
    asset::{io::Reader, AssetLoader, AsyncReadExt, LoadContext},
    input::gamepad,
    log::info,
    math::IVec3,
    render::{
        render_resource::{
            encase::internal::BufferRef, Extent3d, TextureDescriptor, TextureDimension,
            TextureFormat, TextureUsages,
        },
        texture::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    },
    utils::BoxedFuture,
};

use half::f16;
use thiserror::Error;
use vdb_rs::{GridMetadataError, ParseError, VdbReader};

#[derive(Default)]
pub struct VolumeLoader;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum VolumeLoaderError {
    #[error("Failed to read file: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Failed to parse volume: {0}")]
    FailedToParseVolume(#[from] ParseError),
    #[error("Failed to read grid metadata: {0}")]
    GridMetadataError(#[from] GridMetadataError),
}

impl AssetLoader for VolumeLoader {
    type Asset = Image;
    type Settings = ();
    type Error = VolumeLoaderError;
    fn load<'a>(
        &'a self,
        reader: &'a mut Reader,
        _settings: &'a (),
        _load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            // use the file extension for the image type
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            let cursor = std::io::Cursor::new(bytes);
            let mut vdb_reader = VdbReader::new(cursor)?;
            let grid_to_load = vdb_reader.available_grids().first().cloned().unwrap();
            let grid = vdb_reader.read_grid::<half::f16>(&grid_to_load)?;
            let aabb_max = grid.descriptor.aabb_max()?;
            let aabb_min = grid.descriptor.aabb_min()?;

            let aabb = aabb_max - aabb_min + IVec3::new(1, 1, 1);
            dbg!(aabb);
            let size: Extent3d = Extent3d {
                width: aabb.x as u32,
                height: aabb.y as u32,
                depth_or_array_layers: aabb.z as u32,
            };

            dbg!(isize::MAX);

            let mut image_data: Vec<u8> = Vec::new();
            image_data.resize(
                (size.width as u64 * size.height as u64 * size.depth_or_array_layers as u64 * 2)
                    as usize,
                0,
            );

            // Iterate over the grid and fill the pixels
            grid.iter().for_each(|(pos, value)| {
                let x = (pos.x - aabb_min.x as f32) as usize;
                let y = (pos.y - aabb_min.y as f32) as usize;
                let z = (pos.z - aabb_min.z as f32) as usize;
                // info!("x: {}, y: {}, z: {}", x, y, z);
                let index =
                    (x + y * size.width as usize + z * size.width as usize * size.height as usize)
                        * 2;
                let bytes = half::f16::to_ne_bytes(value);
                image_data[index] = bytes[0];
                image_data[index + 1] = bytes[1];
            });

            let mut image = Image::default();
            let image_sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                label: None,
                address_mode_u: ImageAddressMode::ClampToEdge,
                address_mode_v: ImageAddressMode::ClampToEdge,
                address_mode_w: ImageAddressMode::ClampToEdge,
                mag_filter: ImageFilterMode::Linear,
                min_filter: ImageFilterMode::Linear,
                mipmap_filter: ImageFilterMode::Linear,
                ..Default::default()
            });
            image.sampler = image_sampler;
            image.texture_descriptor = TextureDescriptor {
                size,
                dimension: TextureDimension::D3,
                format: TextureFormat::R16Float,
                mip_level_count: 1,
                sample_count: 1,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                label: None,
                view_formats: &[],
            };
            image.data = image_data;
            image.reinterpret_size(size);

            Ok(image)
        })
    }

    fn extensions(&self) -> &[&str] {
        &["vdb"]
    }
}
