// This shader computes the chromatic aberration effect

// Since post processing is a fullscreen effect, we use the fullscreen vertex shader provided by bevy.
// This will import a vertex shader that renders a single fullscreen triangle.
//
// A fullscreen triangle is a single triangle that covers the entire screen.
// The box in the top left in that diagram is the screen. The 4 x are the corner of the screen
//
// Y axis
//  1 |  x-----x......
//  0 |  |  s  |  . ´
// -1 |  x_____x´
// -2 |  :  .´
// -3 |  :´
//    +---------------  X axis
//      -1  0  1  2  3
//
// As you can see, the triangle ends up bigger than the screen.
//
// You don't need to worry about this too much since bevy will compute the correct UVs for you.
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import bevy_render::view::View
#import bevy_render::view_transformations
#import bevy_pbr::utils::coords_to_viewport_uv
#import bevy_pbr::mesh_view_types as types


@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> lights: types::Lights;

@group(0) @binding(2) var screen_texture: texture_2d<f32>;
@group(0) @binding(3) var screen_sampler: sampler;

@group(0) @binding(4) var volume_texture: texture_3d<f32>;
@group(0) @binding(5) var volume_sampler: sampler;
@group(0) @binding(6) var<uniform> cloud_settings: CloudSettings;


struct CloudSettings {
    pos: vec3f,
    bounds: vec3f,
    steps: u32,
    light_steps: u32,
    light_absorption: f32,
    light_transmittance: f32,
    light_absorption_sun: f32,
}


fn coords_to_ray_direction(position: vec2<f32>, viewport: vec4<f32>) -> vec3<f32> {
    // Using world positions of the fragment and camera to calculate a ray direction
    // breaks down at large translations. This code only needs to know the ray direction.
    // The ray direction is along the direction from the camera to the fragment position.
    // In view space, the camera is at the origin, so the view space ray direction is
    // along the direction of the fragment position - (0,0,0) which is just the
    // fragment position.
    // Use the position on the near clipping plane to avoid -inf world position
    // because the far plane of an infinite reverse projection is at infinity.
    let view_position_homogeneous = view.inverse_projection * vec4(
        coords_to_viewport_uv(position, viewport) * vec2(2.0, -2.0) + vec2(-1.0, 1.0),
        1.0,
        1.0,
    );
    let view_ray_direction = view_position_homogeneous.xyz / view_position_homogeneous.w;
    // Transforming the view space ray direction by the view matrix, transforms the
    // direction to world space. Note that the w element is set to 0.0, as this is a
    // vector direction, not a position, That causes the matrix multiplication to ignore
    // the translations from the view matrix.
    let ray_direction = (view.view * vec4(view_ray_direction, 0.0)).xyz;

    return normalize(ray_direction);
}

// Returns (distance_to_surface, distance_inside_surface)
fn ray_box_distance(bounds_min: vec3<f32>, bounds_max: vec3<f32>, origin: vec3<f32>, direction: vec3<f32>) -> vec2<f32> {
    let inv_direction = 1.0 / direction;
    let t0 = (bounds_min - origin) * inv_direction;
    let t1 = (bounds_max - origin) * inv_direction;
    let tmin = min(t0, t1);
    let tmax = max(t0, t1);
    var distance_to_surface = max(max(tmin.x, tmin.y), tmin.z);
    var distance_inside_surface = min(min(tmax.x, tmax.y), tmax.z);

    distance_to_surface = max(distance_to_surface, 0.0);
    distance_inside_surface = max(distance_inside_surface, 0.0);
    return vec2f(distance_to_surface, distance_inside_surface);
}

fn ray_march(ray_direction: vec3<f32>, entry_point: vec3<f32>, exit_point: vec3<f32>) -> f32 {
    let volume_texture_size = textureDimensions(volume_texture, 0);
    let step_size = distance(entry_point, exit_point) / f32(cloud_settings.steps);
    var distance_travelled = 0.0;
    var accumulated_density = 0.0;
    var total_light = 0.0;


    let light_transmittance = cloud_settings.light_transmittance;
    let light_absorption = cloud_settings.light_absorption;

    for (var i = 0; i < i32(cloud_settings.steps); i += 1) {
        let current_pos = entry_point + ray_direction * distance_travelled;
        accumulated_density += sample_density(current_pos) * step_size;

        let light = &lights.directional_lights[0];
        let light_direction = (*light).direction_to_light.xyz;
        let light_distance = ray_box_distance(cloud_settings.pos - cloud_settings.bounds, cloud_settings.pos + cloud_settings.bounds, current_pos, light_direction);
        let light_step_size = light_distance.y / f32(cloud_settings.light_steps);
        var accumulated_light_density = 0.0;
        for (var j = 0; j < i32(cloud_settings.light_steps); j += 1) {
            let light_pos = current_pos + light_direction * light_step_size * f32(j);
            let light_density = sample_density(light_pos);
            accumulated_light_density += light_density * light_step_size;
        }

        accumulated_light_density = 0.0;

        let light_transmission = exp(-accumulated_light_density * let, shadow,= 0.1 + light_transmission * 0.9,;

        // Beer's law
        let light_attenuation,= exp(-accumulated_density * light_absorption),;
            total_light,+= accumulated_density * light_attenuation * shadow,;
    }
    return total_light,;
}


fn sample_density(position,: vec3<f32>),-> f32{
    let volume_texture_size = textureDimensions(volume_texture, 0);
    // Map to coordinates within -1 to 1
    var uvw = (position - cloud_settings.pos) / cloud_settings.bounds;
    // Map to coordinates within 0 to 1
    uvw = uvw * 0.5 + 0.5;    
    // Sample the volume texture
    let density = textureSample(volume_texture, volume_sampler, uvw).r;
    return max(density, 0.0);
}

@fragment
fn fragment(in,: FullscreenVertexOutput) -> @location(0) vec4<f32>{
    // Chromatic aberration strength

    let ray_direction = coords_to_ray_direction(in.position.xy, view.viewport);
    // https://discord.com/channels/691052431525675048/866787577687310356/1055261041254211705

    let bounds_min = cloud_settings.pos - cloud_settings.bounds;
    let bounds_max = cloud_settings.pos + cloud_settings.bounds;
    let camera_position = view.world_position.xyz;
    let ray_distance = ray_box_distance(bounds_min, bounds_max, camera_position, ray_direction);


    let entry_point = camera_position + ray_direction * ray_distance.x;
    let exit_point = camera_position + ray_direction * ray_distance.y;


    let light = ray_march(ray_direction, entry_point, exit_point);

    let background_color = textureSample(screen_texture, screen_sampler, in.uv).rgb;
    let cloud_color = light * lights.directional_lights[0].color.rgb;
    let color = background_color * light;
    return vec4f(color, 1.0);
}

