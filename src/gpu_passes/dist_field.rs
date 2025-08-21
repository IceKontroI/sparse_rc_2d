use bevy::prelude::*;
use bevy::render::{render_graph::*, render_resource::*};
use crate::gpu_api::{attach::*, pass::*, utils::*};
use crate::gpu_resources::textures::*;

#[derive(Default, Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct DistField;

impl Pass for DistField {
    // B side must always be written to last so it can be bound here 
    type Binds = ViewBind<JumpFloodA>;
    type Count = Count<1>;
    type Commands = ();
}

impl Raster for DistField {
    // distance field is the 3rd element in the CoreTextures bind group resource
    type Targets = FromAttach<CoreBindGroup, 2>;
    const VERTEX_FRAGMENT_SHADER_PATH: &'static str = "shaders/dist_field.wgsl";

    fn fragment_targets() -> Vec<Option<ColorTargetState>> {
        vec![Some(CoreBindGroup::color_target_state::<2>())]
    }
}
