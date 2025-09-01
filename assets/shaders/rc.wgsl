////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
// CONSTANTS ///////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

const TASK_VISUALIZER: u32 = 1u;
const PROBE_DUPLICATE_MODE: u32 = 2u;
const CASCADE_BLOCK_MODE: u32 = 3u;
const CASCADE_INTERVAL_MODE: u32 = 4u;
const DISTANCE_FIELD_MODE: u32 = 5u;
const RAY_DEBUG_MODE: u32 = 6u;

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
// COMMON BINDINGS /////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

// render/debug settings
@group(0) @binding(0) var<uniform> function_mode: u32;
@group(0) @binding(1) var<uniform> debug_mode: u32;
@group(0) @binding(2) var<uniform> push_mode: u32;
@group(0) @binding(3) var<uniform> rc_model: u32;

// mouse drawing related
@group(0) @binding(4) var<uniform> mouse_brush_rgba: vec4f;
@group(0) @binding(5) var<uniform> mouse_brush_size: f32;
@group(0) @binding(6) var<uniform> mouse_button_pressed: u32;
@group(0) @binding(7) var<uniform> mouse_last_pos: vec2f;
@group(0) @binding(8) var<uniform> mouse_this_pos: vec2f;

// RC general context
@group(0) @binding(9) var<uniform> screen_dims: vec2u;
@group(0) @binding(10) var<uniform> cascade_dims: vec2u;
@group(0) @binding(11) var<uniform> num_cascades: u32;
@group(0) @binding(12) var<uniform> texel_span: u32;

// RC level context
@group(0) @binding(13) var<uniform> cascade_index: u32;
@group(0) @binding(14) var<uniform> level: array<LevelParams, 32u>;

struct LevelParams {
    two_pow_index: u32,
    angle_ratio: f32,
    probe_spacing: u32,
    interval_start: u32,
}

// core bind group
@group(1) @binding(0)
var scene_albedo: texture_2d<f32>;
@group(1) @binding(1)
var scene_emissive: texture_2d<f32>;
@group(1) @binding(2)
var distance_field: texture_2d<f32>;
@group(1) @binding(3)
var debug_texture: texture_storage_2d<rgba8unorm, read_write>;
@group(1) @binding(4)
var<storage, read_write> statistics: Statistics;
@group(1) @binding(5)
var<storage, read_write> ray_deferred_args: DrawArgs;
@group(1) @binding(6)
var<storage, read_write> ray_vertex_buffer: array<vec4f, 20000000u>; // TODO hardcoded and copied in shader, must change both!

struct DrawArgs {
    vertex_count: u32,
    instance_count: atomic<u32>, // only this will update, and will be reset each frame
    first_vertex: u32,
    first_instance: u32,
}

struct Statistics {
    merge_count: atomic<u32>,
    data_lost: atomic<u32>,
    c0_tasks: atomic<u32>,
    ray_hits: atomic<u32>,
    slabs_allocated: atomic<u32>,
    rays_per_level: array<atomic<u32>, 32u>,
    threads_active: atomic<u32>,
    threads_idle: atomic<u32>,
    debug_ray_count: atomic<u32>,
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
// SAMPLING PATTERNS ///////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

fn inMousePreviewRadius(xy: vec2i) -> bool {
    if function_mode == RAY_DEBUG_MODE {
        // disable preview radius in ray debug mode to prevent ray occlusion around the mouse
        return false;
    }
    return abs(xy.x - i32(mouse_this_pos.x)) < i32(mouse_brush_size) 
        && abs(xy.y - i32(mouse_this_pos.y)) < i32(mouse_brush_size)
        && distance(vec2f(xy), mouse_this_pos) < mouse_brush_size;
}

fn loadAlbedo(xy: vec2i) -> vec4f {
    if inMousePreviewRadius(xy) {
        switch debug_mode {
            case 2u { return vec4f(0.0, 0.0, 0.0, 1.0); } // light
            case 3u { return vec4f(0.0, 0.0, 0.0, 0.0); } // erase
            default { return mouse_brush_rgba;          } // solid
        }
    } else {
        return textureLoad(scene_albedo, xy, 0);
    }
}

fn loadEmissive(xy: vec2i) -> vec4f {
    if inMousePreviewRadius(xy) {
        switch debug_mode {
            case 2u { return mouse_brush_rgba;          } // light
            default { return vec4f(0.0, 0.0, 0.0, 0.0); } // solid or erase
        }
    } else {
        return textureLoad(scene_emissive, xy, 0);
    }
}

// Sampling pattern for indexing into higher mip-levels.
// By sampling the diagonals we always include the current quad and the 3 neighboring quads.
// ╔════╤════╦════╤════╗
// ║ TL ╎    ║ TR ╎    ║
// ╟╶╶╶╶┼╶╶╶╶╫╶╶╶╶┼╶╶╶╶╢
// ║    ╎ XY ║    ╎    ║
// ╠════╪════╬════╪════╣
// ║ BL ╎    ║ BR ╎    ║
// ╟╴╴╴╴┼╶╶╶╶╫╶╶╶╶┼╶╶╶╶╢
// ║    ╎    ║    ╎    ║
// ╚════╧════╩════╧════╝
const DIAG_OFFSETS_LEN: i32 = 4;
const DIAG_OFFSETS: array<vec2i, 4> = array<vec2i, 4>(
    vec2i(-1,-1),
    vec2i( 1,-1),
    vec2i(-1, 1),
    vec2i( 1, 1),
);

// Samples in a quad pattern.
const QUAD_OFFSETS_LEN: i32 = 4;
const QUAD_OFFSETS: array<vec2i, 4> = array<vec2i, 4>(
    vec2i(0, 0),
    vec2i(0, 1),
    vec2i(1, 0),
    vec2i(1, 1),
);

/// Samples in a ring around (not including) the initial coordinate.
const RING_OFFSETS_LEN: i32 = 8;
const RING_OFFSETS: array<vec2i, RING_OFFSETS_LEN> = array<vec2i, RING_OFFSETS_LEN>(
    vec2i(-1,-1),
    vec2i(-1, 0),
    vec2i(-1, 1),
    vec2i( 0,-1),
    vec2i( 0, 1),
    vec2i( 1,-1),
    vec2i( 1, 0),
    vec2i( 1, 1),
);

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
// VERTEX/FRAGMENT /////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

fn fullscreenQuadCorner(corner: u32) -> vec4f {
    switch corner {
        case 0u: { return vec4f(-1.0, -1.0, 0.0, 1.0); } // bottom-left
        case 1u: { return vec4f( 1.0, -1.0, 0.0, 1.0); } // bottom-right
        case 2u: { return vec4f(-1.0,  1.0, 0.0, 1.0); } // top-left
        default: { return vec4f( 1.0,  1.0, 0.0, 1.0); } // top-right
    };
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
// RAY MARCHING ////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

const EPSILON: f32 = 0.70710678118; // 0.5 * √2
const TAU: f32 = 6.28318530717958647692528676655900577;

struct TaskResult {
    direct: vec3f,
    hit: bool,
    merge_xy: vec2u,
    is_merge: bool,
}

fn completeTask(xy: vec2u, c: u32) -> array<TaskResult, 4u> {

    let l: LevelParams = level[c];
    let linear_resolution = cascade_dims / l.two_pow_index;
    let coord_within_block: vec2u = xy % linear_resolution;
    let dir_block_index: vec2u = xy / linear_resolution;
    let origin: vec2u = (coord_within_block * l.probe_spacing) + (l.probe_spacing / 2u);
    let dir_index: u32 = (dir_block_index.x + dir_block_index.y * l.two_pow_index) * 4u;

    // isolates current cascade's contribution by discarding color of other cascades
    let discard_cascade = function_mode == CASCADE_INTERVAL_MODE && c != debug_mode;
    var debug_rays = false;
    if function_mode == RAY_DEBUG_MODE {
        let mouse_xy: vec2u = (vec2u(mouse_this_pos) / 2u) / l.two_pow_index;
        let mouse_coord_within_block: vec2u = mouse_xy % linear_resolution;
        debug_rays = all(mouse_coord_within_block == coord_within_block);
    }

    var task_results: array<TaskResult, 4u>;
    for (var r = 0u; r < 4u; r += 1u) {
        task_results[r] = raymarch(c, origin, dir_index, r, coord_within_block, debug_rays);
        if discard_cascade {
            task_results[r].direct = vec3f(0.0, 0.0, 0.0);
        }
    }
    return task_results;
}

// My struggle with eliminating gaps on the edges of solids was only partially successful:
const T_START: f32 = 0.5;
const EXTRA_LEN: f32 = 0.5;
const ORIGIN_OFFSET: f32 = 0.5;
const ANGLE_OFFSET: f32 = 0.5;

fn raymarch(c: u32, origin: vec2u, dir_index: u32, r: u32, coord_within_block: vec2u, debug: bool) -> TaskResult {

    var task_result = TaskResult(vec3(0), false, getMergeTexelAt(c, dir_index + r, coord_within_block), false);

    // "forking fix": makes all rays from a ray task start at the same position
    // similar to mytino's shadertoy example (but possibly different logic) https://www.shadertoy.com/view/4clcWn
    // here we do it by using 1.5 (middle) instead of r (0/1/2/3) to figure out the origin point
    // but seems to introduce blind spots, so optionally you can enable it by pressing space-bar
    let direction_offset = select(f32(r), 1.5, push_mode == 1u);

    let l: LevelParams = level[c];
    let preavg_dir_index = f32(dir_index) + direction_offset;
    let theta = (preavg_dir_index + ANGLE_OFFSET) * l.angle_ratio;
    let delta = vec2f(cos(theta), sin(theta));
    let ray_origin = vec2f(origin) - ORIGIN_OFFSET + (delta * (f32(l.interval_start) + EXTRA_LEN));
    
    // "nearest neighbor" or "nearest fix" reprojects rays to prevent gaps between the rays that will eventually get merged
    // https://github.com/Yaazarai/GMShaders-Radiance-Cascades/blob/main/RadianceCascades-Optimized/shaders/Shd_RadianceCascades_NearestFix/Shd_RadianceCascades_NearestFix.fsh
    // `n1` variables are for the parent of the probe for which we are casting rays
    let l_n1: LevelParams = level[c + 1u];
    let linear_resolution_n1 = cascade_dims / l_n1.two_pow_index;
    let coord_within_block_n1: vec2u = task_result.merge_xy % linear_resolution_n1;
    let dir_block_index_n1: vec2u = task_result.merge_xy / linear_resolution_n1;
    let origin_n1: vec2u = (coord_within_block_n1 * l_n1.probe_spacing) + (l_n1.probe_spacing / 2u);
    // adding 1.5 to direction index will reproject the child rays to the center of the 4 parent ray it will merge with
    // similar to the way the forking fix logic works, we're reprojecting towards the 1.5 position instead of 0/1/2/3
    let dir_index_n1 = 1.5 + f32((dir_block_index_n1.x + dir_block_index_n1.y * l_n1.two_pow_index) * 4u);
    let theta_n1 = (dir_index_n1 + ANGLE_OFFSET) * l_n1.angle_ratio;
    let delta_n1 = vec2f(cos(theta_n1), sin(theta_n1));
    let ray_target = vec2f(origin_n1) - ORIGIN_OFFSET + (delta_n1 * (f32(l_n1.interval_start) + EXTRA_LEN));
    
    // bend the ray to face the merge location and use that as a stopping position
    let max_distance = length(ray_target - ray_origin);
    let direction = normalize(ray_target - ray_origin);
    var d: f32;

    var debug_index: u32;
    if debug {
        debug_index = atomicAdd(&ray_deferred_args.instance_count, 1u);
        atomicAdd(&statistics.debug_ray_count, 1u);
        ray_vertex_buffer[debug_index].x = ray_origin.x;
        ray_vertex_buffer[debug_index].y = ray_origin.y;
    }

    var start = select(0.0, T_START, c == 0u);
    for (var t = T_START; t <= max_distance; t += d) {
        let ray = vec2i(round(ray_origin + direction * t));
        if any(ray < vec2i(0)) || any(ray >= vec2i(screen_dims)) {
            break;
        }
        d = textureLoad(distance_field, ray, 0).r;
        if d <= EPSILON {
            // fast pseudo-interpolation
            task_result.direct = max(
                max(loadEmissive(ray + vec2i(0, 0)).rgb,
                    loadEmissive(ray + vec2i(0, 1)).rgb,
                ),
                max(loadEmissive(ray + vec2i(1, 1)).rgb,
                    loadEmissive(ray + vec2i(1, 0)).rgb,
                ),
            );
            task_result.hit = true;
            if debug {
                // line endpoint is the point of occlusion
                ray_vertex_buffer[debug_index].z = f32(ray.x);
                ray_vertex_buffer[debug_index].w = f32(ray.y);
            }            
            return task_result;
        }
    }

    if debug {
        // line endpoint is the interval endpoint
        ray_vertex_buffer[debug_index].z = ray_target.x;
        ray_vertex_buffer[debug_index].w = ray_target.y;
    }

    // becomes true if we didn't hit and this isn't the last cascade
    // which enables a downstream merge process for its parent probe
    task_result.is_merge = c + 1u < num_cascades;
    return task_result;
}

fn getMergeTexelAt(c: u32, preavg_dir_index: u32, coord_within_block: vec2u) -> vec2u {
    let two_pow_index_n1: u32 = 1u << (c + 1u);
    let dir_block_size_n1: vec2u = cascade_dims / two_pow_index_n1; 
    let block_offset: vec2u = vec2u(
        preavg_dir_index % two_pow_index_n1,
        preavg_dir_index / two_pow_index_n1,
    ) * dir_block_size_n1;
    let position_index: vec2u = coord_within_block / 2u;
    return block_offset + position_index;
}
