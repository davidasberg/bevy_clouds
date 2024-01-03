use bevy::{
    prelude::*,
    reflect::TypeUuid,
    render::{
        extract_component::ExtractComponent,
        extract_resource::ExtractResource,
        render_resource::{AsBindGroup, ShaderType},
    },
};

// This is the component that will get passed to the shader
#[derive(Component, Default, Clone, ExtractComponent, Copy, ShaderType, Reflect)]
#[reflect(Component)]
pub struct CloudSettings {
    // The size of the cloud volume
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    // The number of steps to take when raymarching
    pub steps: u32,
    // The number of steps to take when raymarching the light
    pub light_steps: u32,

    // The extinction coefficient, sigma_t
    // is the sum of the light scattering and light absorption coefficients

    // The light scattering, sigma_s
    pub light_scattering: f32,
    // The light absorption, sigma_a
    pub light_absorption: f32,

    // The darkness threshold
    pub darkness_threshold: f32,
    // Ray offset strength
    pub ray_offset_strength: f32,

    // The following settings are used in the phase function
    pub base_brightness: f32,
    pub phase_factor: f32,
}

// Rust side (custom material, names irrelevant):
#[derive(AsBindGroup, Asset, Resource, ExtractResource, Clone, Debug, TypePath, TypeUuid)]
#[uuid = "659154e7-5d76-4711-9849-c2e973d2c5f4"]
pub struct CloudSettingsAsset {
    pub alpha_mode: AlphaMode,
    #[uniform(0)]
    pub light_radius: f32,
    #[uniform(1)]
    pub player_position: Vec3,
    #[uniform(2)]
    pub hexling_positions: [Vec3; 2],
}
