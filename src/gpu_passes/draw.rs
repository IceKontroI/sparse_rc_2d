use bevy::prelude::*;
use bevy::render::{render_graph::*, render_resource::*, renderer::*};
use gputil::{attach::*, bind::*, color::*, raster::*, utils::*};
use crate::gpu_resources::{mouse_trail::*, textures::*};

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
    
    type Binds = DrawUniformBind;
    type Count = DrawIter;
    type Commands = ();
    type ColorTargets = SceneAttachments;
    type DepthTarget = ();
    type RasterDraw = RasterDrawQuad;

    fn fragment_targets() -> Vec<Option<ColorTargetState>> {vec![
        Some(CoreBindGroup::color_target_state::<0>()), // albedo 
        Some(CoreBindGroup::color_target_state::<1>()), // emissive
    ]}
    
}

pub struct DrawIter;
impl PassIter for DrawIter {
    type WorldParams<'w, 's> = Res<'w, MouseTrail>;
    type ViewParams<'w, 's> = ();

    fn iterations<'w, 's>(mouse_trail: Res<MouseTrail>, _: ()) -> usize {
        if let MouseTrail { 
            connected: true, 
            last_quad: Some(_),
            .. 
        } = *mouse_trail { 
            1 
        } else { 
            0 
        }
    }
}

pub struct DrawUniformBind;
impl Bind for DrawUniformBind {
    type WorldParams<'w, 's> = Res<'w, MouseTrail>;
    type ViewParams<'w, 's> = ();

    fn layout(device: &RenderDevice) -> BindGroupLayout {
        Uniform::<DrawUniform>::bind_group_layout(device)
    }

    fn group(_: usize, mouse_trail: Res<MouseTrail>, _: (), c: BindContext) -> Option<OOM<BindGroup>> {
        let MouseTrail { 
            last_quad: Some(quad), brush, rgb, .. 
        } = *mouse_trail else {
            // error!("Invalid: {:?}", *mouse_trail);
            return None;
        };
        let uniform = Uniform::of(DrawUniform { quad, brush, rgb, });
        let bind_group = uniform.as_bind_group(c.layout, c.device, c.bind_params).ok()?.bind_group;
        Some(OOM::One(bind_group))
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
