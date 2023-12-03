use std::io::BufReader;

use bevy::{
    asset::{io::Reader, AssetLoader, AsyncReadExt, LoadContext},
    input::gamepad,
    log::info,
    render::{
        render_resource::{encase::internal::BufferRef, Extent3d, TextureDimension, TextureFormat},
        texture::Image,
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
            let aabb = aabb_max - aabb_min;
            let size: Extent3d = Extent3d {
                width: aabb.x as u32,
                height: aabb.y as u32,
                depth_or_array_layers: aabb.z as u32,
            };

            // grid currently contains only voxels that are not "empty"
            // we need to convert it to a Vec<u8> with the correct size
            // Initialize the 3d pixel array, with 0s
            let mut pixels: Vec<Vec<Vec<half::f16>>> =
                vec![
                    vec![
                        vec![f16::from_f32(0.0); size.depth_or_array_layers as usize];
                        size.height as usize
                    ];
                    size.width as usize
                ];

            // Iterate over the grid and fill the pixels
            grid.iter().for_each(|(pos, value)| {
                let x = pos.x as usize;
                let y = pos.y as usize;
                let z = pos.z as usize;
                pixels[x][y][z] = value;
            });

            // Convert the pixels to a 1d array of u8's
            let pixel = pixels
                .into_iter()
                .flatten()
                .flatten()
                .map(|v| half::f16::to_ne_bytes(v))
                .flatten()
                .collect::<Vec<u8>>();

            // Create the image
            let image = Image::new(size, TextureDimension::D3, pixel, TextureFormat::R16Float);

            Ok(image)
        })
    }

    fn extensions(&self) -> &[&str] {
        &["vdb"]
    }
}
