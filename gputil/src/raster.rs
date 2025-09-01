use std::{any::*, marker::*, ops::Range, sync::*};
use bevy::{ecs::{query::*, system::*}, mesh::*, prelude::*, shader::*};
use bevy::render::{diagnostic::*, render_graph::{Node, *}, render_resource::*, renderer::*};
use crate::{bind::*, color::*, utils::*};
use super::depth::*;

pub trait Raster: Sized + Send + Sync + 'static {
    type Binds: Bindings;
    type Count: PassIter;
    type Commands: GpuCommands;
    type ColorTargets: ColorTargets;
    type DepthTarget: DepthTarget;
    type RasterDraw: RasterDraw;

    const VERTEX_FRAGMENT_SHADER_PATH: &'static str;
    const VERTEX_ENTRY_POINT: &'static str = "vertex";
    const FRAGMENT_ENTRY_POINT: &'static str = "fragment";
    const PRIMITIVE_TOPOLOGY: PrimitiveTopology = PrimitiveTopology::TriangleStrip;

    fn shader_defs() -> Vec<ShaderDefVal> { vec![] }
    fn multisample() -> MultisampleState { default() }
    fn vertex_buffers() -> Vec<VertexBufferLayout> { vec![] }
    fn depth_stencil() -> Option<DepthStencilState> { None }
    // TODO integrate this with the type attachment declaration and warn if it doesn't match up (I encountered cryptic error from missing this)
    fn fragment_targets() -> Vec<Option<ColorTargetState>> { vec![] }
}

// TODO this is kinda gross with the borrowing and the many lifetimes but it works so I can't complain
pub trait RasterDraw {
    type WorldParams<'w, 's>: for<'a, 'b> ReadOnlySystemParam<Item<'a, 'b> = Self::WorldParams<'a, 'b>>;
    type ViewParams<'w, 's>: for<'a, 'b> ReadOnlyQueryData<Item<'a, 'b> = Self::ViewParams<'a, 'b>>;

    fn get_raster_draw_type<'a, 'w, 's>(
        world_params: &'a Self::WorldParams<'w, 's>, 
        view_params: &'a Self::ViewParams<'w, '_>,
    ) -> Option<Vec<RasterDrawType<'a>>>;
}

pub struct RasterDrawQuad;
impl RasterDraw for RasterDrawQuad {
    type WorldParams<'w, 's> = ();
    type ViewParams<'w, 's> = ();

    fn get_raster_draw_type<'a, 'w, 's>(_: &'a (), _: &'a ()) -> Option<Vec<RasterDrawType<'a>>> {
        Some(vec![RasterDrawType::SingleQuad])
    }
}

#[derive(Clone)]
pub enum RasterDrawType<'a> {
    FixedVertices { 
        vertices: Range<u32>, 
        instances: Range<u32>,
    },
    SingleQuad,
    SetVertexBuffer {
        slot: u32, 
        buffer_slice: BufferSlice<'a>,
    },
    DrawIndirect {
        indirect_buffer: Buffer, 
        indirect_offset: BufferAddress,
    },
    MultiDrawIndirect {
        indirect_buffer: Buffer, 
        indirect_offset: BufferAddress,
        count: u32,
    },
    SetViewport {
        x: f32, y: f32, w: f32, h: f32, 
        min_depth: f32, max_depth: f32,
    },
    SetScissorRect {
        x: u32, y: u32, width: u32, height: u32
    },
}

pub type WorldRasterParams<'w, 's, T> = (
    <<T as Raster>::Binds as Bindings>::WorldParams<'w, 's>,
    <<T as Raster>::ColorTargets as ColorTargets>::WorldParams<'w, 's>,
    <<T as Raster>::DepthTarget as DepthTarget>::WorldParams<'w, 's>,
    <<T as Raster>::RasterDraw as RasterDraw>::WorldParams<'w, 's>,
    <<T as Raster>::Count as PassIter>::WorldParams<'w, 's>,
    <<T as Raster>::Commands as GpuCommands>::WorldParams<'w, 's>,
);

pub type ViewRasterParams<'w, 's, T> = (
    <<T as Raster>::Binds as Bindings>::ViewParams<'w, 's>,
    <<T as Raster>::ColorTargets as ColorTargets>::ViewParams<'w, 's>,
    <<T as Raster>::DepthTarget as DepthTarget>::ViewParams<'w, 's>,
    <<T as Raster>::RasterDraw as RasterDraw>::ViewParams<'w, 's>,
    <<T as Raster>::Count as PassIter>::ViewParams<'w, 's>,
    <<T as Raster>::Commands as GpuCommands>::ViewParams<'w, 's>,
);

#[derive(Resource)]
pub struct RasterPipeline<T: Raster> {
    layouts: <T::Binds as Bindings>::Layout,
    system_state: Arc<Mutex<SystemState<WorldRasterParams<'static, 'static, T>>>>,
    id: CachedRenderPipelineId,
}

impl<T: Raster> FromWorld for RasterPipeline<T> {
    fn from_world(world: &mut World) -> Self {
        let name = type_name::<Self>();
        let layouts = T::Binds::layout(world.resource::<RenderDevice>());
        let system_state = Arc::new(Mutex::new(SystemState::new(world)));
        let descriptor = RenderPipelineDescriptor {
            label: Some(name.into()),
            layout: T::Binds::layout_vec(&layouts),
            vertex: VertexState {
                shader: world.load_asset(T::VERTEX_FRAGMENT_SHADER_PATH), 
                shader_defs: T::shader_defs(),
                entry_point: Some(T::VERTEX_ENTRY_POINT.into()), 
                buffers: T::vertex_buffers(),
            },
            primitive: PrimitiveState { 
                topology: T::PRIMITIVE_TOPOLOGY,
                cull_mode: None,
                ..default()
            },
            fragment: Some(FragmentState { 
                shader: world.load_asset(T::VERTEX_FRAGMENT_SHADER_PATH), 
                shader_defs: T::shader_defs(),
                entry_point: Some(T::FRAGMENT_ENTRY_POINT.into()), 
                targets: T::fragment_targets(),
            }),
            depth_stencil: T::depth_stencil(),
            multisample: T::multisample(),
            push_constant_ranges: vec![],
            zero_initialize_workgroup_memory: true,
        };
        let id = world.resource_mut::<PipelineCache>().queue_render_pipeline(descriptor);
        info!("Pipeline Created: {name}");
        Self { layouts, system_state, id }
    }
}

impl<T: Raster> Node for RasterPassLabel<T> where
    for<'w, 's> <T::Binds as Bindings>::ViewParams<'w, 's>: NonViewParams,
    for<'w, 's> <T::ColorTargets as ColorTargets>::ViewParams<'w, 's>: NonViewParams,
    for<'w, 's> <T::DepthTarget as DepthTarget>::ViewParams<'w, 's>: NonViewParams,
    for<'w, 's> <T::RasterDraw as RasterDraw>::ViewParams<'w, 's>: NonViewParams,
    for<'w, 's> <T::Count as PassIter>::ViewParams<'w, 's>: NonViewParams,
    for<'w, 's> <T::Commands as GpuCommands>::ViewParams<'w, 's>: NonViewParams,
{
    fn run<'w>(
        &self, graph: &mut RenderGraphContext, context: &mut RenderContext<'w>, world: &'w World
    ) -> Result<(), NodeRunError> {
        <Self as ViewNode>::run(&self, graph, context, default(), world)
    }
}

impl<T: Raster> ViewNode for RasterPassLabel<T> {
    type ViewQuery = ViewRasterParams<'static, 'static, T>;

    fn run<'w>(
        &self, 
        _: &mut RenderGraphContext, 
        context: &mut RenderContext<'w>, 
        (v_bind, v_color, v_depth, v_draw, v_count, v_cmd): ViewRasterParams<'w, '_, T>, 
        world: &'w World
    ) -> Result<(), NodeRunError> {

        let pipelines = world.resource::<PipelineCache>();
        let cached_pipeline = world.resource::<RasterPipeline<T>>();
        let Some(pipeline) = pipelines.get_render_pipeline(cached_pipeline.id) else {
            warn!("Missing {}", type_name::<RasterPipeline::<T>>());
            return Ok(());
        };

        let name = type_name::<T>();

        let mut system_state = cached_pipeline.system_state.lock().unwrap();
        let (w_bind, w_color, w_depth, w_draw, w_count, w_cmd) = system_state.get(world);
        let iterations = T::Count::iterations(w_count, v_count);

        let device = context.render_device();
        let bind_params = &mut get_bind_params(world);
        let layouts = &cached_pipeline.layouts;
        let Some(bind_group) = T::Binds::group(iterations, layouts, device, w_bind, v_bind, bind_params) else {
            return Ok(());
        };
        let Some(color_views) = T::ColorTargets::get_views(iterations, w_color, v_color, bind_params) else {
            return Ok(());
        };
        let depth_view = T::DepthTarget::get_view(iterations, w_depth, v_depth, bind_params);

        let record = context.diagnostic_recorder();
        let commands = context.command_encoder();
        let time_span = record.time_span(commands, name);

        T::Commands::pre_iter(commands, w_cmd, v_cmd);

        let Some(raster_draw) = T::RasterDraw::get_raster_draw_type(&w_draw, &v_draw) else {
            return Ok(());
        };

        for i in 0..iterations {

            let Some(color_attachments) = T::ColorTargets::attachments(&color_views, i) else {
                error!("No ColorTarget attachments provided for {} at iteration {i}/{iterations}", name);
                continue;
            };
            let color_attachments = color_attachments.into_iter().map(|a| Some(a)).collect::<Vec<_>>();
            let color_attachments = color_attachments.as_slice();

            let depth_stencil_attachment = depth_view.as_ref()
                .map(|views| T::DepthTarget::depth_attachment(views, i))
                .flatten();

            let mut render_pass = commands.begin_render_pass(&RenderPassDescriptor {
                label: Some(name),
                color_attachments,
                depth_stencil_attachment,
                ..default()
            });

            render_pass.set_pipeline(pipeline);

            for g in 0..T::Binds::LEN {
                let bind_group = T::Binds::get_group(&bind_group, i, g);
                render_pass.set_bind_group(g, bind_group, &[]);
            }

            for command in &raster_draw {
                match command.clone() {
                    RasterDrawType::FixedVertices { vertices, instances } => {
                        render_pass.draw(vertices, instances)
                    }
                    RasterDrawType::SingleQuad => {
                        render_pass.draw(0..4, 0..1)
                    },
                    RasterDrawType::SetVertexBuffer { slot, buffer_slice } => {
                        render_pass.set_vertex_buffer(slot, *buffer_slice)
                    },
                    RasterDrawType::DrawIndirect { indirect_buffer, indirect_offset } => {
                        render_pass.draw_indirect(&indirect_buffer, indirect_offset)
                    },
                    RasterDrawType::MultiDrawIndirect { indirect_buffer, indirect_offset, count, } => {
                        render_pass.multi_draw_indirect(&indirect_buffer, indirect_offset, count)
                    }
                    RasterDrawType::SetViewport { x, y, w, h, min_depth, max_depth } => {
                        render_pass.set_viewport( x, y, w, h, min_depth, max_depth)
                    },
                    RasterDrawType::SetScissorRect { x, y, width, height } => {
                        render_pass.set_scissor_rect(x, y, width, height)
                    },
                }
            }
        }

        time_span.end(commands);
        Ok(())
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone, ShaderType)]
pub struct IndirectDrawArgs {
    pub vertex_count: u32, 
    pub instance_count: u32,
    pub first_vertex: u32,
    pub first_instance: u32,
}

impl IndirectDrawArgs {
    pub fn points() -> Self {
        Self { vertex_count: 1, ..default() }
    }
    pub fn lines() -> Self {
        Self { vertex_count: 2, ..default() }
    }
    pub fn quads() -> Self {
        Self { vertex_count: 4, ..default() }
    }
}
