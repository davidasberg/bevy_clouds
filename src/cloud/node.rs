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

use super::CloudVolume;
use super::{pipeline::CloudPipeline, settings::CloudSettings};

// The post process node used for the render graph
#[derive(Default)]
pub struct CloudRenderNode;
impl CloudRenderNode {
    pub const NAME: &'static str = "volumetric_clouds";
}

// The ViewNode trait is required by the ViewNodeRunner
impl ViewNode for CloudRenderNode {
    // The node needs a query to gather data from the ECS in order to do its rendering,
    // but it's not a normal system so we need to define it manually.
    //
    // This query will only run on the view entity
    type ViewQuery = (
        &'static ViewTarget,
        &'static ViewUniformOffset,
        &'static ViewLightsUniformOffset,
    );

    // Runs the node logic
    // This is where you encode draw commands.
    //
    // This will run on every view on which the graph is running.
    // If you don't want your effect to run on every camera,
    // you'll need to make sure you have a marker component as part of [`ViewQuery`]
    // to identify which camera(s) should run the effect.
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, view_uniform_offset, view_lights_uniform_offset): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        // info!("Running cloud render node");

        // Get the pipeline resource that contains the global data we need
        // to create the render pipeline
        let cloud_pipeline = world.resource::<CloudPipeline>();

        // The pipeline cache is a cache of all previously created pipelines.
        // It is required to avoid creating a new pipeline each frame,
        // which is expensive due to shader compilation.
        let pipeline_cache = world.resource::<PipelineCache>();

        // Get the pipeline from the cache
        let Some(pipeline) = pipeline_cache.get_render_pipeline(cloud_pipeline.pipeline_id) else {
            return Ok(());
        };

        // Get the mesh_view_bindings layout entries
        let view_uniforms = world.resource::<ViewUniforms>();
        let Some(view_uniforms) = view_uniforms.uniforms.binding() else {
            return Ok(());
        };

        let global_light_meta = world.resource::<LightMeta>();
        let Some(light_binding) = global_light_meta.view_gpu_lights.binding() else {
            return Ok(());
        };

        let Some(cloud) = world.get_resource::<CloudVolume>() else {
            return Ok(());
        };

        let Some(texture) = world
            .resource::<RenderAssets<Image>>()
            .get(cloud.image.clone())
        else {
            // info!("Resource exists but is not loaded yet");
            return Ok(());
        };

        // This will start a new "post process write", obtaining two texture
        // views from the view target - a `source` and a `destination`.
        // `source` is the "current" main texture and you _must_ write into
        // `destination` because calling `post_process_write()` on the
        // [`ViewTarget`] will internally flip the [`ViewTarget`]'s main
        // texture to the `destination` texture. Failing to do so will cause
        // the current main texture information to be lost.
        let post_process = view_target.post_process_write();

        let settings_uniforms = world.resource::<ComponentUniforms<CloudSettings>>();
        let Some(settings_binding) = settings_uniforms.uniforms().binding() else {
            return Ok(());
        };

        // let Some(settings) = settings.
        //     return Ok(());
        // };

        // The bind_group gets created each frame.
        //
        // Normally, you would create a bind_group in the Queue set,
        // but this doesn't work with the post_process_write().
        // The reason it doesn't work is because each post_process_write will alternate the source/destination.
        // The only way to have the correct source/destination for the bind_group
        // is to make sure you get it during the node execution.
        let post_process_bind_group = render_context.render_device().create_bind_group(
            "cloud_bind_group",
            &cloud_pipeline.post_process_layout,
            // It's important for this to match the BindGroupLayout defined in the PostProcessPipeline
            &BindGroupEntries::sequential((
                // View uniform
                view_uniforms,
                // Global light meta
                light_binding,
                // Make sure to use the source view
                post_process.source,
                // Use the sampler created for the pipeline
                &cloud_pipeline.sampler,
                // Volume texture
                &texture.texture_view,
                // Volume sampler
                &texture.sampler,
                // Cloud settings
                settings_binding,
            )),
        );

        // Begin the render pass
        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("cloud_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                // We need to specify the post process destination view here
                // to make sure we write to the appropriate texture.
                view: post_process.destination,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
        });

        // This is mostly just wgpu boilerplate for drawing a fullscreen triangle,
        // using the pipeline/bind_group created above
        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(
            0,
            &post_process_bind_group,
            &[
                view_uniform_offset.offset,
                view_lights_uniform_offset.offset,
            ],
        );
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}
