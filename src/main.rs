mod camera_controller;
mod volumetric_clouds;
use bevy::{math::vec3, prelude::*};

use bevy_inspector_egui::quick::WorldInspectorPlugin;
use camera_controller::{PanOrbitCamera, PanOrbitCameraPlugin};
use volumetric_clouds::{
    CloudVolume, VolumetricCloudLight, VolumetricCloudPlugin, VolumetricCloudSettings,
};

/// Entry point.
fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                watch_for_changes_override: Some(true),
                ..default()
            }),
            VolumetricCloudPlugin,
            WorldInspectorPlugin::default(),
        ))
        .insert_resource(AmbientLight::NONE)
        .add_systems(Startup, setup)
        .add_systems(Update, rotate_camera)
        .run();
}

/// Spawns all the objects in the scene.
fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Spawn a fog volume with a voxelized version of the Stanford bunny.
    commands
        .spawn(SpatialBundle {
            visibility: Visibility::Visible,
            transform: Transform::from_xyz(0.0, 0.5, 0.0),
            ..default()
        })
        .insert(CloudVolume {
            density_texture: Some(asset_server.load("volumes/bunny.ktx2")),
            density_factor: 1.0,
            // Scatter as much of the light as possible, to brighten the bunny
            // up.
            scattering: 1.0,
            ..default()
        });

    // Spawn a bright directional light that illuminates the cloud well.
    commands
        .spawn(DirectionalLightBundle {
            transform: Transform::from_xyz(1.0, 1.0, -0.3).looking_at(vec3(0.0, 0.5, 0.0), Vec3::Y),
            directional_light: DirectionalLight {
                shadows_enabled: true,
                illuminance: 32000.0,
                ..default()
            },
            ..default()
        })
        // Make sure to add this for the light to interact with the cloud.
        .insert(VolumetricCloudLight);

    // Spawn a camera.
    commands
        .spawn(Camera3dBundle {
            transform: Transform::from_xyz(-0.75, 1.0, 2.0)
                .looking_at(vec3(0.0, 0.0, 0.0), Vec3::Y),
            camera: Camera {
                hdr: true,
                ..default()
            },
            ..default()
        })
        .insert(VolumetricCloudSettings {
            // Make this relatively high in order to increase the cloud quality.
            step_count: 64,
            // Disable ambient light.
            ambient_intensity: 0.0,
            ..default()
        });
}

/// Rotates the camera a bit every frame.
fn rotate_camera(mut cameras: Query<&mut Transform, With<Camera3d>>) {
    for mut camera_transform in cameras.iter_mut() {
        *camera_transform =
            Transform::from_translation(Quat::from_rotation_y(0.01) * camera_transform.translation)
                .looking_at(vec3(0.0, 0.5, 0.0), Vec3::Y);
    }
}
