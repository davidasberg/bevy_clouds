use bevy::{
    asset::{io::Reader, AssetLoader, LoadContext},
    render::texture::Image,
    utils::BoxedFuture,
};

use thiserror::Error;
use vdb_rs::VdbReader;

#[derive(Default)]
pub struct VolumeLoader;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum VolumeLoaderError {
    #[error("Failed to load volume: {0}")]
    FailedToLoadVolume(#[from] std::io::Error),
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
            let image = match reader.path().extension().and_then(|s| s.to_str()) {
                Some("vdb") => {
                    let vdb_reader = VdbReader::new(reader);
                    let grid_names = vdb_reader.grid_names();
                    let grid_to_load = grid_names.first().cloned().unwrap_or_default();

                    return Ok(Image {
                        data: vdb_reader.data,
                        size: vdb_reader.size,
                        format: bevy::render::texture::TextureFormat::R32Uint,
                        ..Default::default()
                    });
                }
                _ => {
                    return Err(VolumeLoaderError::FailedToLoadVolume(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Unsupported image format",
                    )))
                }
            };

            Ok(image)
        })
    }

    fn extensions(&self) -> &[&str] {
        &["vdb"]
    }
}
