use bevy::{prelude::*, render::{render_graph::*, render_resource::*}};
use crate::{gpu_api::{attach::*, pass::*}, gpu_resources::textures::*};
use crate::gpu_api::{color::*, utils::*};

#[derive(Default, Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct DistJfaSeed;

impl Pass for DistJfaSeed {
    type Binds = ViewBind<CoreBindGroup>;
    type Count = Count<1>;
    type Commands = ();
}

impl Raster for DistJfaSeed {
    type Targets = FromAttach<JumpFloodA>;
    const VERTEX_FRAGMENT_SHADER_PATH: &'static str = "shaders/dist_jfa_seed.wgsl";
    
    fn fragment_targets() -> Vec<Option<ColorTargetState>> {
        vec![Some(JumpFloodA::color_target_state::<0>())]
    }
}

impl AsTextureView for JumpFloodA {
    fn as_texture_view(&self, bind_params: &mut BindParams<'_>) -> Option<TextureView> {
        bind_params.texture_view(self)
    }
}
