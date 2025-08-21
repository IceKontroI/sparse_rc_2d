use bevy::prelude::*;
use bevy::render::render_graph::*;
use bevy::shader::ShaderDefVal;
use crate::gpu_api::pass::*;
use crate::gpu_api::utils::*;
use crate::gpu_resources::textures::*;
use crate::gpu_resources::{slab::*, uniforms::*};
use crate::core::constants::*;

#[derive(Default, Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct RcSparse;

impl Pass for RcSparse {
    type Binds = (
        WorldBind<RcUniforms>,
        ViewBind<CoreBindGroup>,
        ViewBind<DirectLightingStorageB>,
        WorldBind<Slabs>,
    );
    type Count = Self;
    type Commands = ();
}

impl Compute for RcSparse {
    type Workgroups = Self;
    const COMPUTE_SHADER_PATH: &'static str = "shaders/rc_sparse.wgsl";

    fn shader_defs() -> Vec<ShaderDefVal> {
        vec![
            ShaderDefVal::UInt("BANDWIDTH".into(), BANDWIDTH as u32),
            ShaderDefVal::UInt("SLAB_CAPACITY".into(), SLAB_CAPACITY as u32),
        ]
    }
}

/// We dispatch one workgroup per possible cascade that could appear on screen.
/// 1920x1080 has 6 cascades, so a cascade covers a 64x64 texel area.
/// 30 workgroups is 1920 texels wide, and 16 is 1024 which falls short in height.
/// So for cases like this, we dispatch an extra workgroup to ensure coverage, in this case 30x17.
impl WorkgroupArgs for RcSparse {
    type WorldParams<'w, 's> = Res<'w, RcUniforms>;
    type ViewParams<'w, 's> = ();

    fn workgroups(rcu: Res<RcUniforms>, _: (),) -> UVec3 {
        let dispatch = (rcu.screen_dims + UVec2::splat((rcu.texel_span as u32).saturating_sub(1))) / rcu.texel_span;
        UVec3::new(dispatch.x, dispatch.y, 1)
    }
}

impl PassIter for RcSparse {
    type WorldParams<'w, 's> = Res<'w, RcEnum>;
    type ViewParams<'w, 's> = ();

    fn iterations(rcu: Res<RcEnum>, _: ()) -> usize {
        match *rcu {
            RcEnum::SparseFilled | RcEnum::SparseEdge => 1,
            _ => 0,
        }
    }
}
