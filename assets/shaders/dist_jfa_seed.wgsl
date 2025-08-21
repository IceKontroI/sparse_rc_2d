#import "shaders/rc.wgsl" as rc

@group(0) @binding(0)
var scene_albedo: texture_2d<f32>;
@group(0) @binding(1)
var scene_emissive: texture_2d<f32>;

@vertex
fn vertex(@builtin(vertex_index) corner: u32) -> @builtin(position) vec4f {
    return rc::fullscreenQuadCorner(corner);
}

@fragment
fn fragment(@builtin(position) position: vec4f) -> @location(0) vec2u {
    let a = textureLoad(scene_albedo, vec2u(position.xy), 0).a;
    return select(vec2u(position.xy), vec2u(4294967295u), a == 0.0);
}
