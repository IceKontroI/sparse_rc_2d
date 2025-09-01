use bevy::{mesh::*, prelude::*};
use bevy::render::{render_asset::*, render_graph::*, render_resource::*, storage::*};
use gputil::{attach::*, raster::*, utils::*};
use crate::gpu_resources::{textures::*, uniforms::*};

#[derive(Default, Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct RayDebug;

impl Raster for RayDebug {
    type Binds = WorldBind<RcUniforms>;
    type Count = Count<1>;
    type Commands = ();
    type ColorTargets = FromAttach<CoreBindGroup, 3>; // 3rd item is the debug texture
    type DepthTarget = ();
    type RasterDraw = Self;

    const VERTEX_FRAGMENT_SHADER_PATH: &'static str = "shaders/ray_debug.wgsl";
    const PRIMITIVE_TOPOLOGY: PrimitiveTopology = PrimitiveTopology::LineList;

    fn fragment_targets() -> Vec<Option<ColorTargetState>> {
        vec![Some(CoreBindGroup::color_target_state::<3>())] // debug texture
    }

    fn vertex_buffers() -> Vec<VertexBufferLayout> {
        vec![
            VertexBufferLayout::from_vertex_formats(
                VertexStepMode::Instance, // iterates over the same vertex twice
                vec![VertexFormat::Float32x4],
            )
        ]
    }
}

impl RasterDraw for RayDebug {
    type WorldParams<'w, 's> = Res<'w, RenderAssets<GpuShaderStorageBuffer>>;
    type ViewParams<'w, 's> = &'w CoreBindGroup;

    fn get_raster_draw_type<'a, 'w, 's>(
        buffers: &'a Self::WorldParams<'w, 's>, 
        core_bind: &'a Self::ViewParams<'w, '_>,
    ) -> Option<Vec<RasterDrawType<'a>>> {
        let buffer_slice = buffers.get(&core_bind.ray_vertex_buffer)?.buffer.slice(..);
        let indirect_buffer = buffers.get(&core_bind.ray_deferred_args)?.buffer.clone();
        Some(vec![
            RasterDrawType::SetVertexBuffer { slot: 0, buffer_slice },
            RasterDrawType::DrawIndirect { indirect_buffer, indirect_offset: 0 }
        ])
    }
}
