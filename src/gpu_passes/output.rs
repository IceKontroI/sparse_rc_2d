use std::marker::*;
use bevy::prelude::*;
use bevy::render::{render_graph::*, render_resource::*, view::*};
use gputil::{color::*, raster::*, utils::*};
use crate::gpu_resources::{textures::*, uniforms::*};

#[derive(Default, Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct Output;

impl Raster for Output {
    
    const VERTEX_FRAGMENT_SHADER_PATH: &'static str = "shaders/output.wgsl";
    
    type Binds = (
        WorldBind<RcUniforms>,
        ViewBind<CoreBindGroup>, 
        ViewBind<DirectLightingB>,
    );
    type ColorTargets = Screen;
    type DepthTarget = ();
    type Count = Count<1>;
    type Commands = ();
    type RasterDraw = RasterDrawQuad;

    fn fragment_targets() -> Vec<Option<ColorTargetState>> {
        vec![Some(TextureFormat::bevy_default().into())]
    }
}

pub struct Screen;
impl ColorTarget for Screen {
    type WorldParams<'w, 's> = ();
    type ViewParams<'w, 's> = &'w ViewTarget;

    fn get_view(_: usize, _: (), view: &ViewTarget, _: &mut BindParams<'_>) -> Option<OOM<TextureView>> { 
        Some(OOM::One(view.post_process_write().destination.clone()))
    }
}
