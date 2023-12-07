use bevy::{
    core_pipeline::{core_3d, fullscreen_vertex_shader::fullscreen_shader_vertex_state},
    ecs::query::QueryItem,
    pbr::{GpuLights, LightMeta, ViewLightsUniformOffset},
    prelude::*,
    render::{
        extract_component::{
            ComponentUniforms, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin,
        },
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::RenderAssets,
        render_graph::{
            NodeRunError, RenderGraphApp, RenderGraphContext, ViewNode, ViewNodeRunner,
        },
        render_resource::{
            BindGroupEntries, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
            BindingType, BufferBindingType, CachedRenderPipelineId, ColorTargetState, ColorWrites,
            FragmentState, MultisampleState, Operations, PipelineCache, PrimitiveState,
            RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor, Sampler,
            SamplerBindingType, SamplerDescriptor, ShaderStages, ShaderType, TextureFormat,
            TextureSampleType, TextureViewDimension,
        },
        renderer::{RenderContext, RenderDevice},
        texture::BevyDefault,
        view::{ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms},
        RenderApp,
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
    // The light absorption
    pub light_absorption: f32,

    // The light absorption towards the sun
    pub light_absorption_sun: f32,
    // The darkness threshold
    pub darkness_threshold: f32,
    // Ray offset strength
    pub ray_offset_strength: f32,

    // The following settings are used in the phase function
    pub forward_scattering: f32,
    pub back_scattering: f32,
    pub base_brightness: f32,
    pub phase_factor: f32,
}
