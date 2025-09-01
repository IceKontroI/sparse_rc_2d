use bevy::{prelude::*, render::{render_graph::*, render_resource::*}};
use gputil::{attach::*, color::*, raster::*, utils::*};
use crate::gpu_resources::{textures::*, uniforms::RcUniforms};

#[derive(Default, Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct DistJfaSeed;

impl Raster for DistJfaSeed {

    const VERTEX_FRAGMENT_SHADER_PATH: &'static str = "shaders/dist_jfa_seed.wgsl";

    type Binds = (
        WorldBind<RcUniforms>,
        ViewBind<CoreBindGroup>,
    );
    type ColorTargets = FromAttach<JumpFloodA>;
    type DepthTarget = ();
    type Count = Count<1>;
    type Commands = ();
    type RasterDraw = RasterDrawQuad;
    
    fn fragment_targets() -> Vec<Option<ColorTargetState>> {
        vec![Some(JumpFloodA::color_target_state::<0>())]
    }
}

impl AsTextureView for JumpFloodA {
    fn as_texture_view(&self, bind_params: &mut BindParams<'_>) -> Option<TextureView> {
        bind_params.texture_view(self)
    }
}
