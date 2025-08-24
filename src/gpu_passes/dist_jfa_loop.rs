use bevy::{prelude::*, render::{render_graph::*, render_resource::*, renderer::*}};
use gputil::{attach::*, bind::*, color::*, raster::*, utils::*};
use crate::gpu_resources::{textures::*, uniforms::*};

#[derive(Default, Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct DistJfaLoop;

impl Raster for DistJfaLoop {

    const VERTEX_FRAGMENT_SHADER_PATH: &'static str = "shaders/dist_jfa_loop.wgsl";

    type Binds = (
        JfaUniformBind,
        PingPongJFA,
    );
    type Count = JfaIterations;
    type Commands = ();
    type ColorTargets = PingPongJFA;
    type DepthTarget = ();
    type RasterDraw = RasterDrawQuad;
    
    fn fragment_targets() -> Vec<Option<ColorTargetState>> {
        // Distance A and B are both the same, so we can safely ping-pong between A and B using A's definition
        vec![Some(JumpFloodA::color_target_state::<0>())]
    }
}

pub struct PingPongJFA;
impl Bind for PingPongJFA {
    type WorldParams<'w, 's> = ();
    type ViewParams<'w, 's> = (&'w JumpFloodA, &'w JumpFloodB);

    fn layout(device: &RenderDevice) -> BindGroupLayout {
        // A and B are the same definition, so we just use A here
        JumpFloodA::bind_group_layout(device)
    }

    fn group(iterations: usize, _: (), (a, b): (&JumpFloodA, &JumpFloodB), c: BindContext) -> Option<OOM<BindGroup>> {
        let mut vec = Vec::new();
        let mut ping_pong = true;
        for _ in 0..iterations {
            let bind_group = if ping_pong {
                a.as_bind_group(&c.layout, c.device, c.bind_params).ok()?.bind_group
            } else {
                b.as_bind_group(&c.layout, c.device, c.bind_params).ok()?.bind_group
            };
            vec.push(bind_group);
            ping_pong = !ping_pong;
        }
        Some(OOM::Many(vec))
    }
}

impl ColorTarget for PingPongJFA {
    type WorldParams<'w, 's> = ();
    type ViewParams<'w, 's> = (&'w JumpFloodA, &'w JumpFloodB);

    fn get_view(iterations: usize, _: (), (a, b): Self::ViewParams<'_, '_>, bind_params: &mut BindParams<'_>) -> Option<OOM<TextureView>> {
        let mut vec = Vec::new();
        let mut ping_pong = true;
        for _ in 0..iterations {
            vec.push(if ping_pong {
                bind_params.texture_view(b)?
            } else {
                bind_params.texture_view(a)?
            });
            ping_pong = !ping_pong;
        }
        Some(OOM::Many(vec))
    }
}

pub struct JfaUniformBind;
impl Bind for JfaUniformBind {
    type WorldParams<'w, 's> = ();
    type ViewParams<'w, 's> = ();

    fn layout(device: &RenderDevice) -> BindGroupLayout {
        Uniform::<u32>::bind_group_layout(device)
    }

    fn group(iterations: usize, _: (), _: (), c: BindContext) -> Option<OOM<BindGroup>> {
        let mut vec = Vec::new();
        for i in (0..iterations as u32).rev() {
            let u = Uniform::of(1u32 << i);
            vec.push(u.as_bind_group(c.layout, c.device, c.bind_params).ok()?.bind_group);
        }
        Some(OOM::Many(vec))
    }
}

pub struct JfaIterations;
impl PassIter for JfaIterations {
    type WorldParams<'w, 's> = Res<'w, RcUniforms>;
    type ViewParams<'w, 's> = ();

    fn iterations(rcu: Res<RcUniforms>, _: ()) -> usize {
        let mut iterations = f32::log2(rcu.screen_dims.max_element() as f32).ceil() as usize;
        // ensure the output always ends up on the same side for even and odd iterations
        // odd passes do an extra iteration so we end up on the "A" side, like the evens
        iterations += iterations % 2;
        iterations      
    }
}
