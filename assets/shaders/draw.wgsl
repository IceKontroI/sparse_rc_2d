#import "shaders/rc.wgsl" as rc

const MOUSE_TRAIL_POINTS: u32 = #{MOUSE_TRAIL_POINTS};

struct VertexOut {
    @builtin(position) pos: vec4f,
    @location(0) center_screen: vec2f,
};

@vertex
fn vertex(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> VertexOut {
    var offset: vec2f;
    switch vertex % 4u {
        case 0u: { offset = vec2f(-1.0, -1.0); }
        case 1u: { offset = vec2f( 1.0, -1.0); }
        case 2u: { offset = vec2f(-1.0,  1.0); }
        default: { offset = vec2f( 1.0,  1.0); }
    }
    offset *= rc::mouse_brush_size;
    let screen_pos = mix(
        rc::mouse_last_pos, 
        rc::mouse_this_pos, 
        f32(instance) / f32(MOUSE_TRAIL_POINTS - 1u),
    );
    let norm = (screen_pos + offset) / vec2f(rc::screen_dims);
    let pos = vec2f(norm.x * 2.0 - 1.0, 1.0 - norm.y * 2.0);
    var out: VertexOut;
    out.pos = vec4f(pos, 0.0, 1.0);
    out.center_screen = screen_pos;
    return out;
}

@fragment
fn fragment(
    @builtin(position) frag_pos: vec4f, 
    @location(0) center_screen: vec2f,
) -> FragmentOut {

    if rc::mouse_button_pressed == 0u {
        discard;
    }

    let frag_screen = frag_pos.xy;
    if distance(frag_screen, center_screen) > rc::mouse_brush_size {
        discard;
    }

    var out: FragmentOut;
    switch rc::debug_mode {
        case 2u { // light
            out.color = vec4f(0.0, 0.0, 0.0, 1.0);
            out.emissive = rc::mouse_brush_rgba;
        }
        case 3u { // erase
            out.color = vec4f(0.0, 0.0, 0.0, 0.0);
            out.emissive = vec4f(0.0, 0.0, 0.0, 0.0);
        }
        default { // solid
            out.color = rc::mouse_brush_rgba;
            out.emissive = vec4f(0.0, 0.0, 0.0, 0.0);
        }
    }
    return out;
}

struct FragmentOut {
    @location(0) color: vec4f,
    @location(1) emissive: vec4f,
};
