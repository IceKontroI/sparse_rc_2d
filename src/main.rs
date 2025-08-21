use bevy::app::*;
use rc::debug::timings::*;
use rc::gpu_passes::plugin::*;
use rc::gpu_resources::{mouse_trail::*, slab::*, textures::*, uniforms::*};
use rc::utils::{launch::*, save_load::*};

/// TODO backlog:
/// * performance bottleneck in the sparse shader where threads can be very idle in some scenes, fix needs major rework
/// * discrepancy in color between sparse and dense model, caused by sparse model's Rgba8Unorm color compression
///   Bevy's output uses Rgba8UnormSrgb, so compressing to Rgba8Unorm before applying it to the screen causes this
/// * saving an image has a texel wrapping error if the window has been resized previously
/// * saved images are darker but when loaded are correct: caused by color space normalization
/// * distance field takes very long to build and is dense, which is against the spirit of Sparse RC
///   but reusing the same ray-marching for dense and sparse makes the two models more comparable
/// * sparse model outputs lighting to a dense texture which then gets applied to the screen in in `output.wgsl`, 
///   but we can directly write to the screen in sparse shader by binding the screen's output as a storage texture 
///   by adding `CameraMainTextureUsages::default().with(TextureUsages::STORAGE_BINDING)` to the camera

fn main() {
    App::new()
        .add_plugins(Launch::<1920, 1080>)
        .add_plugins(MouseTrailPlugin)
        .add_plugins(TexturesPlugin)
        .add_plugins(RenderPassTimingsPlugin)
        .add_plugins(UniformsPlugin)
        .add_plugins(SlabPlugin)
        .add_plugins(SaveLoadPlugin)
        .add_plugins(RenderPassesPlugin)
        .run();
}
