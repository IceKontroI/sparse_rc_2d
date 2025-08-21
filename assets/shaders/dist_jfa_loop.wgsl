#import "shaders/rc.wgsl" as rc

@group(0) @binding(0)
var<uniform> jump_dist: u32;

// NOTE we are reading from A/B and writing to the flip side
@group(1) @binding(0)
var dist_a_or_b: texture_2d<u32>;

@vertex
fn vertex(@builtin(vertex_index) corner: u32) -> @builtin(position) vec4f {
    return rc::fullscreenQuadCorner(corner);
}

@fragment
fn fragment(@builtin(position) position: vec4f) -> @location(0) vec2u {
    let xy = position.xy;
    let xy_i = vec2i(xy);
    let bounds = vec2i(textureDimensions(dist_a_or_b));
    var closest_xy = textureLoad(dist_a_or_b, xy_i, 0).rg;
    var closest_dist = distance(vec2f(closest_xy), xy);
    for (var i = 0u; i < 8u; i += 1u) {
        let jump = xy_i + rc::RING_OFFSETS[i] * i32(jump_dist);
        if any(jump < vec2i(0)) || any(jump >= bounds) {
            continue; // out of bounds creates artifacts at (0, 0)
        }
        let test_xy = textureLoad(dist_a_or_b, jump, 0).rg;
        let dist = distance(vec2f(test_xy), position.xy);
        if dist < closest_dist {
            closest_dist = dist;
            closest_xy = test_xy;
        }
    }
    return closest_xy;
}
