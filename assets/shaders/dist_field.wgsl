#import "shaders/rc.wgsl" as rc

// A side is always written to last
@group(0) @binding(0)
var jfa_dist_a: texture_2d<u32>;

@vertex
fn vertex(@builtin(vertex_index) corner: u32) -> @builtin(position) vec4f {
    return rc::fullscreenQuadCorner(corner);
}

@fragment
fn fragment(@builtin(position) position: vec4f) -> @location(0) f32 {
    let closest_xy = textureLoad(jfa_dist_a, vec2u(position.xy), 0).rg;
    return distance(position.xy, vec2f(closest_xy));
}
