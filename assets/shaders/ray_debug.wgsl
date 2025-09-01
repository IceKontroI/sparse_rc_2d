// @group(0) @binding(1) var<uniform> debug_mode: u32;
// @group(0) @binding(9) var<uniform> screen_dims: vec2u;
// @group(0) @binding(8) var<uniform> mouse_this_pos: vec2f;

// @vertex
// fn vertex(
//     @builtin(vertex_index) vertex_instance: u32,    
//     @location(0) input: vec4f,
// ) -> VertexOut {
//     let xy = select(input.zw, input.xy, vertex_instance % 2u == 0u);
//     let norm = xy / vec2f(screen_dims);
//     var out: VertexOut;
//     let clip = vec4f(norm.x * 2.0 - 1.0, 1.0 - norm.y * 2.0, 0.0, 1.0);
//     out.pos = clip;
//     return out;
// }

// struct VertexOut {
//     @builtin(position) pos: vec4f,
// };

// @fragment
// fn fragment() -> @location(0) vec4f {
//     return vec4f(1.0, 1.0, 1.0, 1.0);
// }


@group(0) @binding(1) var<uniform> debug_mode: u32;
@group(0) @binding(9) var<uniform> screen_dims: vec2u;
@group(0) @binding(8) var<uniform> mouse_this_pos: vec2f;

@vertex
fn vertex(
    @builtin(vertex_index) vertex_instance: u32,    
    @location(0) input: vec4f,
) -> VertexOut {
    let xy = select(input.zw, input.xy, vertex_instance % 2u == 0u);
    let norm = xy / vec2f(screen_dims);
    let mouse_norm = mouse_this_pos / vec2f(screen_dims);
    let zoom = 1u << debug_mode;
    let zoomed = mouse_norm + (norm - mouse_norm) * f32(zoom);
    let clip = vec4f(zoomed.x * 2.0 - 1.0, 1.0 - zoomed.y * 2.0, 0.0, 1.0);
    var out: VertexOut;
    out.pos = clip;
    return out;
}

struct VertexOut {
    @builtin(position) pos: vec4f,
};

@fragment
fn fragment() -> @location(0) vec4f {
    return vec4f(1.0, 1.0, 1.0, 1.0);
}