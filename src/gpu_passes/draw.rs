use bevy::prelude::*;
use bevy::render::{render_graph::*, render_resource::*};
use gputil::{attach::*, color::*, raster::*, utils::*};
use crate::gpu_resources::{textures::*, uniforms::*};

const MOUSE_TRAIL_POINTS: u32 = 64;

#[derive(Default, Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct Draw;

#[derive(Default, Copy, Clone, ShaderType)]
pub struct DrawUniform {
    quad: [Vec4; 4],
    brush: u32,
    rgb: Vec3,
}

impl Raster for Draw {
    
    const VERTEX_FRAGMENT_SHADER_PATH: &'static str = "shaders/draw.wgsl";
    
    type Binds = WorldBind<RcUniforms>;
    type Count = Count<1>;
    type Commands = ();
    type ColorTargets = SceneAttachments; // TODO this can be simplified, pretty sure
    type DepthTarget = ();
    type RasterDraw = Self;

    fn shader_defs() -> Vec<ShaderDefVal> {
        vec![ShaderDefVal::UInt("MOUSE_TRAIL_POINTS".into(), MOUSE_TRAIL_POINTS)]
    }

    fn fragment_targets() -> Vec<Option<ColorTargetState>> {vec![
        Some(CoreBindGroup::color_target_state::<0>()), // albedo 
        Some(CoreBindGroup::color_target_state::<1>()), // emissive
    ]}
}

impl RasterDraw for Draw {
    type WorldParams<'w, 's> = ();
    type ViewParams<'w, 's> = ();

    fn get_raster_draw_type<'a, 'w, 's>(
        _: &'a Self::WorldParams<'w, 's>, 
        _: &'a Self::ViewParams<'w, '_>,
    ) -> Option<Vec<RasterDrawType<'a>>> {
        Some(vec![RasterDrawType::FixedVertices { 
            vertices: 0..(4 * MOUSE_TRAIL_POINTS), 
            instances: 0..MOUSE_TRAIL_POINTS,
        }])
    }
}

pub struct SceneAttachments;
impl ColorTargets for SceneAttachments {
    type WorldParams<'w, 's> = ();
    type ViewParams<'w, 's> = &'w CoreBindGroup;
    type Views = [TextureView; 2];
    const LEN: u32 = 2;

    fn get_views<'w, 's>(_: usize, _: (), scene: Self::ViewParams<'w, 's>, bind_params: &mut BindParams<'w>) -> Option<Self::Views> {
        let albedo = bind_params.texture_view::<0>(scene)?;
        let emissive = bind_params.texture_view::<1>(scene)?;
        Some([albedo, emissive])
    }

    fn attachments(views: &[TextureView; 2], _: usize) -> Option<Vec<RenderPassColorAttachment>> {
        views.into_iter().map(|view| Some(view.color_attachment())).collect()
    }
}
