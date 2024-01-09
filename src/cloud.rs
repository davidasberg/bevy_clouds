mod node;
mod pipeline;
pub mod settings;

use bevy::{
    core_pipeline::core_3d,
    prelude::*,
    render::{
        extract_component::{ExtractComponentPlugin, UniformComponentPlugin},
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_graph::{RenderGraphApp, ViewNodeRunner},
        RenderApp,
    },
};

use self::settings::{CloudSettings, CloudSettingsAsset};
use self::{node::CloudRenderNode, pipeline::CloudPipeline};

#[derive(Resource, ExtractResource, Default, Clone)]
struct CloudVolume {
    image: Handle<Image>,
}

fn load_volume(asset_server: Res<AssetServer>, mut commands: Commands) {
    let image: Handle<Image> = asset_server.load("volumes/cloud_010.vdb");
    commands.insert_resource(CloudVolume { image });
    commands.insert_resource(CloudSettingsAsset {
        alpha_mode: AlphaMode::Blend,
        light_radius: 0.5,
        player_position: Vec3::new(0.0, 0.0, 0.0),
        hexling_positions: [Vec3::new(0.0, 0.0, 0.0); 2],
    });
    commands.spawn((
        CloudSettings {
            bounds_min: Vec3::new(-1.0, -1.0, -1.0),
            bounds_max: Vec3::new(1.0, 1.0, 1.0),
            steps: 250,
            light_steps: 20,
            light_scattering: 0.5,
            light_absorption: 25.0,
            darkness_threshold: 0.16,
            ray_offset_strength: 0.015,
            base_brightness: 0.05,
            phase_factor: 0.55,
        },
        Name::new("Cloud Settings"),
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
