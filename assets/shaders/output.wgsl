#import "shaders/rc.wgsl" as rc

@group(2) @binding(0)
var direct_lighting: texture_2d<f32>;

@vertex
fn vertex(@builtin(vertex_index) corner: u32) -> @builtin(position) vec4f {
    return rc::fullscreenQuadCorner(corner);
}

@fragment
fn fragment(@builtin(position) position: vec4f) -> @location(0) vec4f {
    let xy = vec2u(position.xy);
    switch rc::function_mode {
        case rc::TASK_VISUALIZER       { return visualizeTasks(xy); }
        case rc::PROBE_DUPLICATE_MODE  { return drawProbeDuplicates(xy); }
        case rc::DISTANCE_FIELD_MODE   { return drawDistanceField(xy); }
        case rc::CASCADE_BLOCK_MODE    { return textureLoad(rc::debug_texture, xy / 2); }
        case rc::CASCADE_INTERVAL_MODE { return getLighting(xy, false); }
        case rc::RAY_DEBUG_MODE        { return drawCascadeRays(xy); }
        default                        { return drawScene(xy); }
    }
}

fn getLighting(xy: vec2u, stylize: bool) -> vec4f {
    return textureLoad(direct_lighting, vec2i(xy) / 2, 0);
}

fn drawScene(xy: vec2u) -> vec4f {
    let emissive = rc::loadEmissive(vec2i(xy));
    if emissive.a > 0.0 {
        return emissive;
    }
    return rc::loadAlbedo(vec2i(xy)) + getLighting(xy, true);
}

/// Sparse model only. Debugs the tasks that were actually involved in computing the scene's lighting.
/// Each pixel represents a task of a probe and the color indicates how many times (up to 4) its rays hit a solid.
fn visualizeTasks(xy: vec2u) -> vec4f {
    let task_data = vec2u(textureLoad(rc::debug_texture, xy / 2).rg * 255.0);
    if task_data.r == 1u {
        let total_hits = countOneBits(task_data.g);
        switch total_hits {
            case 0u { return vec4f(1.0, 0.0, 0.0, 1.0); } // 0 -> red
            case 1u { return vec4f(1.0, 1.0, 0.0, 1.0); } // 1 -> yellow
            case 2u { return vec4f(0.0, 1.0, 0.0, 1.0); } // 2 -> green
            case 3u { return vec4f(0.0, 1.0, 1.0, 1.0); } // 3 -> cyan
            default { return vec4f(0.0, 0.0, 1.0, 1.0); } // 4 -> blue
        }
    }
    return vec4f(0);
}

/// Sparse model only. Duplicate tasks light up red, normal is blue, sparse is black.
fn drawProbeDuplicates(xy: vec2u) -> vec4f {
    let task_data = vec2u(textureLoad(rc::debug_texture, xy / 2).rg * 255.0);
    switch task_data.r {
        case 0u { return vec4f(0.0, 0.0, 0.0, 1.0); } // 0: empty -> black
        case 1u { return vec4f(0.0, 0.0, 1.0, 1.0); } // 1: single -> blue
        default { return vec4f(1.0, 0.0, 0.0, 1.0); } // duplicated -> red
    }
}

/// Debugs the distance field, drawing white for long distances and black for nearby.
fn drawDistanceField(xy: vec2u) -> vec4f {
    let dist = textureLoad(rc::distance_field, xy, 0).r;
    let size = textureDimensions(rc::distance_field);
    return vec4f(dist / length(vec2f(size)));
}

/// Draws the rays of the cascade used to generate fluence for the c0 probe nearest to the mouse.
/// Note that the zoom is already applied in the vertex shader to the lines so they are higher quality.
fn drawCascadeRays(xy: vec2u) -> vec4f {
    var zoom_xy = xy;
    if rc::function_mode == rc::RAY_DEBUG_MODE {
        let norm = vec2f(xy) / vec2f(rc::screen_dims);
        let mouse_norm = rc::mouse_this_pos / vec2f(rc::screen_dims);
        let zoom = 1u << rc::debug_mode;
        let zoomed = mouse_norm + (norm - mouse_norm) * (1.0 / f32(zoom));
        zoom_xy = vec2u(clamp(zoomed * vec2f(rc::screen_dims), vec2f(0.0), vec2f(rc::screen_dims - 1u)));
    }
    return drawScene(zoom_xy) + textureLoad(rc::debug_texture, xy);
}
