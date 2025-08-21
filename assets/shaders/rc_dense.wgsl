#import "shaders/rc.wgsl" as rc

@group(2) @binding(0)
var direct_lighting: texture_2d<f32>;

@vertex
fn vertex(@builtin(vertex_index) corner: u32) -> @builtin(position) vec4f {
    return rc::fullscreenQuadCorner(corner);
}

@fragment
fn fragment(@builtin(position) position: vec4f) -> @location(0) vec4f {
    
    let c: u32 = rc::cascade_index;
    let xy: vec2u = vec2u(position.xy);
    let task_results = rc::completeTask(xy, c);
    var merges = 0u;
    var out = vec4f(0);
    for (var r = 0u; r < 4u; r += 1u) {
        if task_results[r].hit {
            out += vec4(task_results[r].direct, 1.0);
        }
        if task_results[r].is_merge {
            out += textureLoad(direct_lighting, task_results[r].merge_xy, 0);
            merges += 1u;
        }        
    }
    out *= 0.25;

    // statistics disabled due to massive atomic contention, causing frame times to take ~1.5 ms extra
    // atomicAdd(&rc::statistics.rays_per_level[c], 4u);
    // atomicAdd(&rc::statistics.merge_count, merges);

    // block mode debugs the lighting only for a cascade level, selected with the debug_mode
    if rc::function_mode == rc::CASCADE_BLOCK_MODE && rc::debug_mode == c {
        textureStore(rc::debug_texture, xy, vec4(out.rgb, 1.0));
    }

    return out;
}
