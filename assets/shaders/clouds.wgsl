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
    light_scattering: f32,
    light_absorption: f32,
    darkness_threshold: f32,
    ray_offset_strength: f32,
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

fn light_march(position: vec3f, cos_theta: f32) -> f32 {
    let dir_to_light = lights.directional_lights[0].direction_to_light;
    let ray_distance = ray_box_distance(cloud_settings.bounds_min, cloud_settings.bounds_max, position, dir_to_light);
    let distance_inside_box = ray_distance.y;

    let step_size = distance_inside_box / f32(cloud_settings.light_steps);
    var distance_travelled = 0.0;
    var total_density = 0.0;
    while distance_travelled < distance_inside_box {
        let position = position + dir_to_light * distance_travelled;
        let density = sample_density(position);
        total_density += max(0.0, density * step_size);
        distance_travelled += step_size;
    }

    let transmittance = exp(-total_density * cloud_settings.light_absorption);
    return cloud_settings.darkness_threshold + transmittance * (1.0 - cloud_settings.darkness_threshold);
}

fn random(st: vec2f) -> f32 {
    return fract(sin(dot(st, vec2(12.9898, 78.233))) * 43758.5453123);
}


fn rayleigh_phase_function(cos_theta: f32) -> f32 {
    return (3.0 / 4.0) * (1.0 + cos_theta * cos_theta);
}

fn henyey_greenstein_phase_function(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let denom = 4.0 * PI * pow(1.0 + g2 - 2.0 * g * cos_theta, 1.5);
    return (1.0 - g2) / denom;
}

fn cornette_shanks_phase_function(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let denom = 2.0 * (2.0 + g2) * pow(1.0 + g2 - 2.0 * g * cos_theta, 1.5);
    return (3.0 * (1.0 - g2) * (1.0 + cos_theta * cos_theta)) / denom;
}

fn phase_function(cos_theta: f32) -> f32 {
    let base_brightness = cloud_settings.base_brightness;
    let phase_factor = cloud_settings.phase_factor;
    let phase = henyey_greenstein_phase_function(cos_theta, phase_factor);
    // let phase = cornette_shanks_phase_function(cos_theta, phase_factor);
    // let phase = rayleigh_phase_function(cos_theta);
    return base_brightness + phase;
}

// fn multiple_octave_scattering(density: f32, cos_theta: f32) -> f32 {
//     let attenuation = 0.3; // Attenuation factor
//     let contribution = 0.7; // contribution 
//     let phase_attenuation = 0.9; // eccentricity attenuation
//     let N = 4;

//     var a = 1.0;
//     var b = 1.0;
//     var c = 1.0;
//     var g = 0.85;

//     var luminance = 0.0;
//     for (var i = 0; i < N; i++) {
//         var phase = phase_function(cos_theta, c * cloud_settings.phase_factor);
//         var beers = beers(density * a * cloud_settings.light_absorption);
//         luminance += b * phase * beers;
//         a *= attenuation;
//         b *= contribution;
//         c *= (1.0 - phase_attenuation);
//     }

//     return luminance;
// }

fn beers(d: f32) -> f32 {
    return exp(-d);
}

fn powder(d: f32) -> f32 {
    return (2.0 - exp(-d * 3.0));
}

fn beers_powder(d: f32) -> f32 {
    return beers(d) * powder(d);
}


// sigma_a | Absorption coefficient | 1/m
// sigma_s | Scattering coefficient | 1/m
// sigma_t | Extinction coefficient | 1/m
// sigma_t = sigma_a + sigma_s
// rho | Albedo | unitless
// p | Phase function | 1/sr (inverse steradians)
// L | Luminance | cd/m^2
// L(x,w) | Light at point x in direction w | cd/m^2
// E | Illuminance | cd / (sr * m^2)

// Absorption: photons that are absorbed by the medium
// 
// Out-scattering: photons that are scattered away by bouncing off particles.
// This is done according to the phase function.
// 
// In-scattering: photons that are scattered into the view direction from other directions.
// This is done according to the phase function.
//
// Emittance: photons that are emitted by the medium itself.
// In the case of clouds, this is ignored, as clouds are emissive.


@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    // https://discord.com/channels/691052431525675048/866787577687310356/1055261041254211705
    var ray_origin = view.world_position;
    var ray_direction = pixel_to_ray_direction(saturate(in.uv));
    var ray_distance = ray_box_distance(cloud_settings.bounds_min, cloud_settings.bounds_max, ray_origin, ray_direction);
    let distance_to_box = ray_distance.x;
    let distance_inside_box = ray_distance.y;
    let entry_point = ray_origin + ray_direction * distance_to_box;

    let absorption = cloud_settings.light_absorption;

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
            let light_transmittance = light_march(position, cos_theta);
            light_energy += density * step_size * transmittance * light_transmittance;
            transmittance *= beers(step_size * density * absorption);

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