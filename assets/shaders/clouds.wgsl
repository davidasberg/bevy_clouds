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


const PI = 3.1415926535;

struct CloudSettings {
    bounds_min: vec3f,
    bounds_max: vec3f,
    steps: u32,
    light_steps: u32,
    light_absorption: f32,
    light_absorption_sun: f32,
    darkness_threshold: f32,
    ray_offset_strength: f32,
    forward_scattering: f32,
    back_scattering: f32,
    base_brightness: f32,
    phase_factor: f32,
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

// Returns (distance_to_box, distance_inside_box)
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

    let distance_to_box = max(dstA, 0.0);
    let distance_inside_box = max(dstB - distance_to_box, 0.0);
    return vec2f(distance_to_box, distance_inside_box);

}

fn sample_density(position: vec3<f32>) -> f32 {
    // position is in world space -1 to 1
    // volume texture is in uv space 0 to 1
    var uvw = (position - cloud_settings.bounds_min) / (cloud_settings.bounds_max - cloud_settings.bounds_min);
    // Sample the volume texture
    let density = textureSample(volume_texture, volume_sampler, uvw).x;
    return max(density, 0.0);
}

fn light_march(position: vec3f) -> f32 {
    let dir_to_light = lights.directional_lights[0].direction_to_light;
    let ray_distance = ray_box_distance(cloud_settings.bounds_min, cloud_settings.bounds_max, position, dir_to_light);
    let distance_inside_box = ray_distance.y;

    let step_size = distance_inside_box / f32(cloud_settings.light_steps);
    var distance_travelled = 0.0;
    var total_density = 0.0;
    while distance_travelled < distance_inside_box {
        let position = position + dir_to_light * distance_travelled;
        let density = sample_density(position);
        total_density += max(0.0,density * step_size);
        distance_travelled += step_size;
    }

    let transmittance = exp(-total_density * cloud_settings.light_absorption_sun);
    return cloud_settings.darkness_threshold + transmittance * (1.0 - cloud_settings.darkness_threshold);
}

fn random(st: vec2f) -> f32 {
    return fract(sin(dot(st, vec2(12.9898, 78.233))) * 43758.5453123);
}


fn henyey_greenstein_phase_function(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let denom = 4.0 * PI * pow(1.0+ g2- 2.0 * g * cos_theta, 1.5); 
}

fn phase_function(cos_theta: f32) -> f32 {
    let blend = 0.5;
    let forward_scattering = cloud_settings.forward_scattering;
    let back_scattering = cloud_settings.back_scattering;
    let base_brightness = cloud_settings.base_brightness;
    let phase_factor = cloud_settings.phase_factor;
    let hg_blend = henyey_greenstein_phase_function(cos_theta, forward_scattering) * (1.0-blend) + henyey_greenstein_phase_function(cos_theta, -back_scattering) * blend;
    return base_brightness + phase_factor * hg_blend;
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    // https://discord.com/channels/691052431525675048/866787577687310356/1055261041254211705
    var ray_origin = view.world_position;
    var ray_direction = pixel_to_ray_direction(saturate(in.uv));
    var ray_distance = ray_box_distance(cloud_settings.bounds_min, cloud_settings.bounds_max, ray_origin, ray_direction);
    let distance_to_box = ray_distance.x;
    let distance_inside_box = ray_distance.y;
    let entry_point = ray_origin + ray_direction * distance_to_box;
    
    let random_offset = random(in.uv) * cloud_settings.ray_offset_strength;
    var distance_travelled = random_offset;

    let cos_theta = dot(ray_direction, lights.directional_lights[0].direction_to_light);
    let phase_value = phase_function(cos_theta);
    
    let distance_limit = distance_inside_box;
    let step_size = distance_limit / f32(cloud_settings.steps);
    var light_energy = 0.0;
    var transmittance = 1.0;
    while distance_travelled < distance_limit {
        let position = entry_point + ray_direction * distance_travelled;
        let density = sample_density(position);
        if density > 0.0 {
            let light_transmittance = light_march(position);
            light_energy += density * step_size * transmittance * light_transmittance * phase_value;
            transmittance *= exp(-density * step_size * cloud_settings.light_absorption);

            if transmittance < 0.01 {
                break;
            }
        }
        distance_travelled += step_size;
    }
    // Calculate light absorption
    let background_color = textureSample(screen_texture, screen_sampler, in.uv).rgb;
    let cloud_color = light_energy * lights.directional_lights[0].color.rgb;
    let color = background_color * transmittance + cloud_color;
    return vec4f(color, 0.0);
}