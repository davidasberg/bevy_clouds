use bevy::{
    core_pipeline::{core_3d, fullscreen_vertex_shader::fullscreen_shader_vertex_state},
    ecs::query::QueryItem,
    pbr::{GlobalLightMeta, GpuLights, LightMeta, ViewLightsUniformOffset},
    prelude::*,
    reflect::TypeUuid,
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
            AsBindGroup, BindGroupEntries, BindGroupLayout, BindGroupLayoutDescriptor,
            BindGroupLayoutEntry, BindingType, BufferBindingType, CachedRenderPipelineId,
            ColorTargetState, ColorWrites, FragmentState, IntoBinding, MultisampleState,
            Operations, PipelineCache, PrimitiveState, RenderPassColorAttachment,
            RenderPassDescriptor, RenderPipelineDescriptor, Sampler, SamplerBindingType,
            SamplerDescriptor, ShaderStages, ShaderType, TextureFormat, TextureSampleType,
            TextureViewDimension, UniformBuffer,
        },
        renderer::{RenderContext, RenderDevice},
        settings,
        texture::BevyDefault,
        view::{ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms},
        RenderApp,
    },
};

#[derive(Resource, ExtractResource, Default, Clone)]
struct CloudVolume {
    image: Handle<Image>,
}

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
    // The light scattering
    pub light_transmittance: f32,
    // The light absorption towards the sun
    pub light_absorption_sun: f32,
    // The darkness threshold
    pub darkness_threshold: f32,
}

fn load_volume(asset_server: Res<AssetServer>, mut commands: Commands) {
    let image: Handle<Image> = asset_server.load("volumes/wdas_cloud_sixteenth.vdb");
    commands.insert_resource(CloudVolume { image });
    commands.spawn((
        CloudSettings {
            bounds_min: Vec3::new(-1.0, -1.0, -1.0),
            bounds_max: Vec3::new(1.0, 1.0, 1.0),
            steps: 32,
            light_steps: 8,
            light_absorption: 1.05,
            light_transmittance: 0.1,
            light_absorption_sun: 1.0,
            darkness_threshold: 0.28,
        },
        Name::new("cloud_settings"),
    ));
}

/// It is generally encouraged to set up post processing effects as a plugin
pub struct CloudRenderPlugin;

impl Plugin for CloudRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractComponentPlugin::<CloudSettings>::default(),
            UniformComponentPlugin::<CloudSettings>::default(),
            ExtractResourcePlugin::<CloudVolume>::default(),
        ));

        app.add_systems(Startup, load_volume);
        app.register_type::<CloudSettings>();

        // We need to get the render app from the main app
        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            // Bevy's renderer uses a render graph which is a collection of nodes in a directed acyclic graph.
            // It currently runs on each view/camera and executes each node in the specified order.
            // It will make sure that any node that needs a dependency from another node
            // only runs when that dependency is done.
            //
            // Each node can execute arbitrary work, but it generally runs at least one render pass.
            // A node only has access to the render world, so if you need data from the main world
            // you need to extract it manually or with the plugin like above.
            // Add a [`Node`] to the [`RenderGraph`]
            // The Node needs to impl FromWorld
            //
            // The [`ViewNodeRunner`] is a special [`Node`] that will automatically run the node for each view
            // matching the [`ViewQuery`]
            .add_render_graph_node::<ViewNodeRunner<CloudRenderNode>>(
                // Specify the name of the graph, in this case we want the graph for 3d
                core_3d::graph::NAME,
                // It also needs the name of the node
                CloudRenderNode::NAME,
            )
            .add_render_graph_edges(
                core_3d::graph::NAME,
                // Specify the node ordering.
                // This will automatically create all required node edges to enforce the given ordering.
                &[
                    core_3d::graph::node::END_MAIN_PASS,
                    CloudRenderNode::NAME,
                    core_3d::graph::node::BLOOM,
                ],
            );
    }

    fn finish(&self, app: &mut App) {
        // We need to get the render app from the main app
        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            // Initialize the pipeline
            .init_resource::<CloudPipeline>();
    }
}

// The post process node used for the render graph
#[derive(Default)]
struct CloudRenderNode;
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

        // This will start a new "post process write", obtaining two texture
        // views from the view target - a `source` and a `destination`.
        // `source` is the "current" main texture and you _must_ write into
        // `destination` because calling `post_process_write()` on the
        // [`ViewTarget`] will internally flip the [`ViewTarget`]'s main
        // texture to the `destination` texture. Failing to do so will cause
        // the current main texture information to be lost.
        let post_process = view_target.post_process_write();

        let Some(cloud) = world.get_resource::<CloudVolume>() else {
            return Ok(());
        };

        let Some(texture) = world
            .resource::<RenderAssets<Image>>()
            .get(cloud.image.clone())
        else {
            info!("Resource exists but is not loaded yet");
            return Ok(());
        };

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

// This contains global data used by the render pipeline. This will be created once on startup.
#[derive(Resource)]
struct CloudPipeline {
    post_process_layout: BindGroupLayout,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

impl FromWorld for CloudPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        // We need to define the bind group layout used for our pipeline
        let post_process_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("cloud_bind_group_layout"),
                entries: &[
                    // The view uniform
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::VERTEX_FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: Some(ViewUniform::min_size()),
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: Some(GpuLights::min_size()),
                        },
                        count: None,
                    },
                    // The screen texture
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: true },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // The sampler that will be used to sample the screen texture
                    BindGroupLayoutEntry {
                        binding: 3,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        count: None,
                    },
                    // The volume texture
                    BindGroupLayoutEntry {
                        binding: 4,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: true },
                            view_dimension: TextureViewDimension::D3,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // The sampler that will be used to sample the volume texture
                    BindGroupLayoutEntry {
                        binding: 5,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Cloud settings
                    BindGroupLayoutEntry {
                        binding: 6,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: Some(CloudSettings::min_size()),
                        },
                        count: None,
                    },
                ],
            });

        // We can create the sampler here since it won't change at runtime and doesn't depend on the view
        let sampler = render_device.create_sampler(&SamplerDescriptor::default());

        // Get the shader handle
        let shader = world
            .resource::<AssetServer>()
            .load("shaders/post_processing.wgsl");

        let pipeline_id = world
            .resource_mut::<PipelineCache>()
            // This will add the pipeline to the cache and queue it's creation
            .queue_render_pipeline(RenderPipelineDescriptor {
                label: Some("post_process_pipeline".into()),
                layout: vec![post_process_layout.clone()],
                // This will setup a fullscreen triangle for the vertex state
                vertex: fullscreen_shader_vertex_state(),
                fragment: Some(FragmentState {
                    shader,
                    shader_defs: vec![],
                    // Make sure this matches the entry point of your shader.
                    // It can be anything as long as it matches here and in the shader.
                    entry_point: "fragment".into(),
                    targets: vec![Some(ColorTargetState {
                        format: TextureFormat::bevy_default(),
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                // All of the following properties are not important for this effect so just use the default values.
                // This struct doesn't have the Default trait implemented because not all field can have a default value.
                primitive: PrimitiveState::default(),
                depth_stencil: None,
                multisample: MultisampleState::default(),
                push_constant_ranges: vec![],
            });

        Self {
            post_process_layout,
            sampler,
            pipeline_id,
        }
    }
}
