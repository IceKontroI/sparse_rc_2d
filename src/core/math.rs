use bevy::math::*;
use bevy::render::render_resource::*;
use crate::core::constants::*;

pub fn ceil_to_power_of_n(number: f32, n: f32) -> f32 {
    n.powf(number.log(n).ceil())
}

pub fn ceil_to_multiple_of_n(number: f32, n: f32) -> f32 {
    (number / n).ceil() * n
}

pub fn num_cascades(dimensions: Vec2) -> u32 {
    let num_cascades = Vec2::ZERO.distance(dimensions).log(4.0).ceil() as u32;
    u32::min(num_cascades, MAX_CASCADES as u32)
}

/// The screen's viewport is not guaranteed to be cleanly divisible by the texel size of a cascade block.
/// This upscales the extents of cascade storage textures to accommodate the whole cascade block.
/// Texture size is increased so there is slight memory and performance overhead for Hybrid and Dense models.
/// But Sparse model doesn't need dense storage, so this has no impact on its memory usage or compute times.
pub fn get_cascade_extents(dimensions: UVec2) -> Extent3d {
    let depth_or_array_layers = num_cascades(dimensions.as_vec2());
    let block_size = 1 << depth_or_array_layers;
    let mut extents = dimensions / PROBE_SPACING;
    extents += block_size - 1;
    extents -= extents % block_size;
    let UVec2 { x: width, y: height } = extents;
    Extent3d { width, height, depth_or_array_layers }
}
