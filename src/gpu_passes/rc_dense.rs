use bevy::prelude::*;
use bevy::render::{render_graph::*, render_resource::*, renderer::*};
use crate::core::math::*;
use crate::gpu_api::{attach::*, bind::*, color::*, pass::*, utils::*};
use crate::gpu_resources::{textures::*, uniforms::*};

/// Top-down combined raymarch and merge fragment render pass.
/// Exhaustively casts every ray of every cascade, then merges with the parent.
/// Storage is currently a 2D texture array with length equal to total number of cascades.
/// This could (and should) be switched to a ping-pong process in a production environment.
/// But the Hybrid mode needs a texture array, so we reuse that data structure for simplicity.
#[derive(Default, Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct RcDense;

impl Pass for RcDense {
    type Binds = (
        RcDenseUniformBinds,
        ViewBind<CoreBindGroup>,
        DenseLightBind,
    );
    type Count = Self;
    type Commands = ();
}

impl Raster for RcDense {
    type Targets = DenseLightTarget;
    const VERTEX_FRAGMENT_SHADER_PATH: &'static str = "shaders/rc_dense.wgsl";

    fn fragment_targets() -> Vec<Option<ColorTargetState>> {
        vec![Some(DirectLightingA::color_target_state())]
    }
}

impl PassIter for RcDense {
    type WorldParams<'w, 's> = (Res<'w, RcEnum>, Res<'w, RcUniforms>);
    type ViewParams<'w, 's> = ();

    fn iterations((rc_enum, rc_uniforms): Self::WorldParams<'_, '_>, _: ()) -> usize {
        match *rc_enum {
            RcEnum::Dense => rc_uniforms.num_cascades as usize,
            _ => 0,
        }
    }
}

pub struct DenseLightBind;
impl Bind for DenseLightBind {
    type WorldParams<'w, 's> = ();
    type ViewParams<'w, 's> = (
        &'w DirectLightingA,
        &'w DirectLightingB,
    );

    fn layout(device: &RenderDevice) -> BindGroupLayout {
        DirectLightingA::bind_group_layout(device)
    }

    fn group(iterations: usize, _: (), (a, b): Self::ViewParams<'_, '_>, c: BindContext) -> Option<OOM<BindGroup>> {
        let a = a.as_bind_group(c.layout, c.device, c.bind_params).ok()?.bind_group;
        let b = b.as_bind_group(c.layout, c.device, c.bind_params).ok()?.bind_group;
        let mut vec = Vec::with_capacity(iterations);
        for i in 0..iterations {
            vec.push(match i % 2 {
                0 => a.clone(),
                _ => b.clone(),
            });
        }
        if iterations % 2 == 0 {
            vec.reverse();
        }
        Some(OOM::Many(vec))
    }
}

pub struct DenseLightTarget;
impl ColorTargets for DenseLightTarget {
    type WorldParams<'w, 's> = Res<'w, RcUniforms>;
    type ViewParams<'w, 's> = (
        &'w DirectLightingA,
        &'w DirectLightingB,
    );
    type Views = OOM<TextureView>;
    const LEN: u32 = 1;

    fn get_views(
        iterations: usize, 
        rcu: Self::WorldParams<'_, '_>, 
        (a, b): Self::ViewParams<'_, '_>, 
        bind_params: &mut BindParams<'_>,
    ) -> Option<Self::Views> {

        let mut correct = get_cascade_extents(rcu.screen_dims);
        correct.depth_or_array_layers = 1;

        let a = bind_params.0.get(&a[0])?;
        let b = bind_params.0.get(&b[0])?;
        if a.size != correct || b.size != correct {
            return None;
        }

        let a = a.texture.create_view(&DirectLightingA::texture_view(a.size).descriptor());
        let b = b.texture.create_view(&DirectLightingB::texture_view(b.size).descriptor());

        let mut vec = Vec::with_capacity(iterations);
        for i in 0..iterations {
            vec.push(match i % 2 {
                0 => a.clone(),
                _ => b.clone(),
            });
        }
        // NOTE this is the opposite condition to what Bind impl does
        if iterations % 2 != 0 {
            vec.reverse();
        }
        Some(OOM::Many(vec))
    }

    fn attachments(direct: &OOM<TextureView>, index: usize) -> Option<Vec<RenderPassColorAttachment>> {
        Some(vec![
            RenderPassColorAttachment { 
                view: &direct[index],
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(default()),
                    store: StoreOp::Store
                },
            },
        ])
    }
}

pub struct RcDenseUniformBinds;
impl Bind for RcDenseUniformBinds {
    type WorldParams<'w, 's> = Res<'w, RcUniforms>;
    type ViewParams<'w, 's> = ();

    fn layout(device: &RenderDevice) -> BindGroupLayout {
        RcUniforms::bind_group_layout(device)
    }

    fn group(num_cascades: usize, rcu: Res<RcUniforms>, _: (), c: BindContext) -> Option<OOM<BindGroup>> {
        let mut u = *rcu;
        let mut vec = Vec::new();
        for i in (0..num_cascades).rev() {
            u.cascade_level = i as u32;
            vec.push(u.as_bind_group(c.layout, c.device, c.bind_params).ok()?.bind_group);
        }
        Some(OOM::Many(vec))
    }
}
