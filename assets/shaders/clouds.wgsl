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
    bounds_min: vec3f,
    bounds_max: vec3f,
    steps: u32,
    light_steps: u32,
    light_absorption: f32,
    light_transmittance: f32,
    light_absorption_sun: f32,
}

fn coords_to_ray_direction(position: vec2f, viewport: vec4f) -> vec3f {
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


fn pixel_to_ray_direction(pixel_uv: vec2<f32>) -> vec3<f32> {
    let pixel_ndc = (pixel_uv * 2.0) - 1.0;
    let primary_ray_target = view.inverse_view_proj * vec4(pixel_ndc.x, -pixel_ndc.y, 1.0, 1.0);
    return normalize((primary_ray_target.xyz / primary_ray_target.w) - view.world_position);
}

// Returns (distance_to_surface, distance_inside_surface)
fn ray_box_distance(bounds_min: vec3f, bounds_max: vec3f, origin: vec3f, direction: vec3f) -> vec2f {

    // CASE 1: ray intersects box from outside (0 <= dstA <= dstB)
    // dstA is dst to nearest intersection, dstB dst to far intersection

    // CASE 2: ray intersects box from inside (dstA < 0 < dstB)
    // dstA is the dst to intersection behind the ray, dstB is dst to forward intersection

    // CASE 3: ray misses box (dstA > dstB)

    let inv_direction = 1.0 / direction;
    let t0 = (bounds_min - origin) * inv_direction;
    let t1 = (bounds_max - origin) * inv_direction;
    let tmin = min(t0, t1);
    let tmax = max(t0, t1);
    let dstA = max(max(tmin.x, tmin.y), tmin.z);
    let dstB = min(min(tmax.x, tmax.y), tmax.z);

    return vec2f(dstA, dstB);

}



fn sample_density(position: vec3<f32>) -> f32 {
    // position is in world space -1 to 1
    // volume texture is in uv space 0 to 1
    var density = 0.0;
    if position.x < -1.0 || position.x > 1.0 || position.y < -1.0 || position.y > 1.0 || position.z < -1.0 || position.z > 1.0 {
       density = 0.0;
    }else {
        let uvw = position * 0.5 + 0.5;
        // Sample the volume texture    
        density = textureSample(volume_texture, volume_sampler, uvw).r;
    }
    return density;
}

fn debug_draw_density(uv: vec2<f32>) -> vec4<f32> {
    var channels = vec4f(0.0);
    let depth = 0.0;
    let dimensions = textureDimensions(volume_texture, 0);
    let sample_pos = vec3f(uv * vec2f(dimensions.xy), depth);

    channels = textureSample(volume_texture, volume_sampler, sample_pos);

    return channels;
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    

    // https://discord.com/channels/691052431525675048/866787577687310356/1055261041254211705
    var ray_direction = pixel_to_ray_direction(in.uv);

    let bounds_min = cloud_settings.bounds_min;
    let bounds_max = cloud_settings.bounds_max;
    let camera_position = view.world_position.xyz;

    let ray_distance = ray_box_distance(bounds_min, bounds_max, camera_position, ray_direction);
    let to_box = ray_distance.x;
    let inside_box = ray_distance.y - ray_distance.x;
    let entry_point = camera_position + ray_direction * ray_distance.x;
    let exit_point = camera_position + ray_direction * ray_distance.y;



    let step_size = ray_distance.y / f32(cloud_settings.steps);
    var density = 0.0;

    var distance_limit = ray_distance.y;
    var distance_traveled = 0.0;
    var light_energy = 0.0;
    var transmittance = 1.0;
    while distance_traveled < distance_limit {
        let position = entry_point + ray_direction * distance_traveled;
        let density_sample = sample_density(position);
        if density_sample > 0.0 {
            light_energy += density_sample * step_size * cloud_settings.light_transmittance;
            transmittance *= exp(-density_sample * step_size * cloud_settings.light_absorption);
        }

        if transmittance < 0.01 {
            break;
        }

        distance_traveled += step_size;
    }

    let background_color = textureSample(screen_texture, screen_sampler, in.uv).rgb;
    let cloud_color = light_energy * lights.directional_lights[0].color.rgb;
    var color = background_color * transmittance + cloud_color * (1.0 - transmittance);
    color = saturate(color);
    if inside_box < 0.0 {
        color = vec3f(0.0, 0.0, 0.0);
    }
    return vec4f(color, 1.0);

    // return vec4f(textureSample(volume_texture, volume_sampler, vec3f(in.uv, f32(cloud_settings.steps) / 100.0)).rgb, 1.0);
}

