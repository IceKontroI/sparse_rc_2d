@group(0) @binding(0)
var<uniform> u: DrawUniform;

struct DrawUniform {
    quad: array<vec4f, 4>,
    brush: u32,
    rgb: vec3f,
}

@vertex
fn vertex(@builtin(vertex_index) corner: u32) -> @builtin(position) vec4f {
    return u.quad[corner];
}

@fragment
fn fragment(@builtin(position) position: vec4f) -> Out {
    var out: Out;
    if u.brush == 1u {
        out.color = vec4f(0.0, 0.0, 0.0, 1.0);
        out.emissive = vec4f(u.rgb, 1.0);
    } else {
        out.color = vec4f(u.rgb, 1.0);
        out.emissive = vec4f(0.0, 0.0, 0.0, 0.0);
    }
    return out;
}

struct Out {
    @location(0) color: vec4f,
    @location(1) emissive: vec4f,
};
