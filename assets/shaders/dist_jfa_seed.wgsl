#import "shaders/rc.wgsl" as rc

@vertex
fn vertex(@builtin(vertex_index) corner: u32) -> @builtin(position) vec4f {
    return rc::fullscreenQuadCorner(corner);
}

@fragment
fn fragment(@builtin(position) position: vec4f) -> @location(0) vec2u {
    let sparse = rc::loadAlbedo(vec2i(position.xy)).a == 0.0;
    return select(vec2u(position.xy), vec2u(4294967295u), sparse);
}
