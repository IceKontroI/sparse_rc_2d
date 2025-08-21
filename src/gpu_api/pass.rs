use std::{any::*, marker::*, sync::*};
use bevy::{ecs::{query::*, system::*}, mesh::*, prelude::*, shader::*};
use bevy::render::{diagnostic::*, render_graph::{Node, *}, render_resource::*, renderer::*};
use crate::gpu_api::{bind::*, color::*, utils::*};

// TODO add support for deferred compute args, custom draw commands, and other cool stuff
// TODO raster pass only supports triangle strip quads which is very limiting
// TODO set_bind_group(..., &[]); must support offsets instead of `&[]` for view binds: https://bevyengine.org/examples/shaders/custom-post-processing/
// TODO `depth_stencil_attachment: None` add support for depth/stencil attachments
// TODO pipelines use Arc<Mutex<SystemState>> which will break change detection in some cases, but this low priority

pub trait Pass: Sized + Send + Sync + 'static {
    type Binds: Bindings;
    type Count: PassIter;
    type Commands: GpuCommands;
}

pub trait Raster: Pass {
    type Targets: ColorTargets;
    const VERTEX_FRAGMENT_SHADER_PATH: &'static str;
    const VERTEX_ENTRY_POINT: &'static str = "vertex";
    const FRAGMENT_ENTRY_POINT: &'static str = "fragment";

    fn shader_defs() -> Vec<ShaderDefVal> { vec![] }
    fn multisample() -> MultisampleState { default() }
    fn vertex_buffers() -> Vec<VertexBufferLayout> { vec![] }
    fn depth_stencil() -> Option<DepthStencilState> { None }
    // TODO integrate this with the type attachment declaration and warn if it doesn't match up (I encountered cryptic error from missing this)
    fn fragment_targets() -> Vec<Option<ColorTargetState>> { vec![] }
}

pub trait Compute: Pass {
    type Workgroups: WorkgroupArgs;
    const COMPUTE_SHADER_PATH: &'static str;

    fn shader_defs() -> Vec<ShaderDefVal> { vec![] }
}

pub trait WorkgroupArgs {
    type WorldParams<'w, 's>: for<'a, 'b> ReadOnlySystemParam<Item<'a, 'b> = Self::WorldParams<'a, 'b>>;
    type ViewParams<'w, 's>: for<'a, 'b> ReadOnlyQueryData<Item<'a, 'b> = Self::ViewParams<'a, 'b>>;

    fn workgroups<'w, 's>(
        world_params: Self::WorldParams<'w, 's>, 
        view_params: Self::ViewParams<'w, '_>,
    ) -> UVec3;
}

pub struct WorkgroupDispatch<const X: u32, const Y: u32, const Z: u32>;

impl<const X: u32, const Y: u32, const Z: u32> WorkgroupArgs for WorkgroupDispatch<X, Y, Z> {
    type WorldParams<'w, 's> = ();
    type ViewParams<'w, 's> = ();

    fn workgroups(_: (), _: ()) -> UVec3 {
        UVec3::new(X, Y, Z)
    }
}

pub type WorldRasterParams<'w, 's, T> = (
    <<T as Pass>::Binds as Bindings>::WorldParams<'w, 's>,
    <<T as Raster>::Targets as ColorTargets>::WorldParams<'w, 's>,
    <<T as Pass>::Count as PassIter>::WorldParams<'w, 's>,
    <<T as Pass>::Commands as GpuCommands>::WorldParams<'w, 's>,
);
pub type WorldComputeParams<'w, 's, T> = (
    <<T as Pass>::Binds as Bindings>::WorldParams<'w, 's>,
    <<T as Compute>::Workgroups as WorkgroupArgs>::WorldParams<'w, 's>,
    <<T as Pass>::Count as PassIter>::WorldParams<'w, 's>,
    <<T as Pass>::Commands as GpuCommands>::WorldParams<'w, 's>,
);
pub type ViewRasterParams<'w, 's, T> = (
    <<T as Pass>::Binds as Bindings>::ViewParams<'w, 's>,
    <<T as Raster>::Targets as ColorTargets>::ViewParams<'w, 's>,
    <<T as Pass>::Count as PassIter>::ViewParams<'w, 's>,
    <<T as Pass>::Commands as GpuCommands>::ViewParams<'w, 's>,
);
pub type ViewComputeParams<'w, 's, T> = (
    <<T as Pass>::Binds as Bindings>::ViewParams<'w, 's>,
    <<T as Compute>::Workgroups as WorkgroupArgs>::ViewParams<'w, 's>,
    <<T as Pass>::Count as PassIter>::ViewParams<'w, 's>,
    <<T as Pass>::Commands as GpuCommands>::ViewParams<'w, 's>,
);

#[derive(Resource)]
pub struct RasterPipeline<T: Pass + Raster> {
    layouts: <<T as Pass>::Binds as Bindings>::Layout,
    system_state: Arc<Mutex<SystemState<WorldRasterParams<'static, 'static, T>>>>,
    id: CachedRenderPipelineId,
}

#[derive(Resource)]
pub struct ComputePipeline<T: Pass + Compute> {
    layouts: <<T as Pass>::Binds as Bindings>::Layout,
    system_state: Arc<Mutex<SystemState<WorldComputeParams<'static, 'static, T>>>>,
    id: CachedComputePipelineId,
}

impl<T: Pass + Raster> FromWorld for RasterPipeline<T> {
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
                topology: PrimitiveTopology::TriangleStrip,
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

impl<T: Pass + Compute> FromWorld for ComputePipeline<T> {
    fn from_world(world: &mut World) -> Self {
        let name = type_name::<Self>();
        let layouts = T::Binds::layout(world.resource::<RenderDevice>());
        let system_state = Arc::new(Mutex::new(SystemState::new(world)));
        let descriptor = ComputePipelineDescriptor {
            label: Some(name.into()),
            layout: T::Binds::layout_vec(&layouts),
            shader: world.load_asset(T::COMPUTE_SHADER_PATH),
            entry_point: Some("compute".into()),
            shader_defs: T::shader_defs(),
            push_constant_ranges: vec![],
            zero_initialize_workgroup_memory: true,
        };
        let id = world.resource_mut::<PipelineCache>().queue_compute_pipeline(descriptor);
        info!("Pipeline Created: {name}");
        Self { layouts, system_state, id }
    }
}

impl<T: Pass + Raster> Node for RasterPassLabel<T> where
    for<'w, 's> <T::Binds as Bindings>::ViewParams<'w, 's>: NonViewParams,
    for<'w, 's> <T::Targets as ColorTargets>::ViewParams<'w, 's>: NonViewParams,
    for<'w, 's> <T::Count as PassIter>::ViewParams<'w, 's>: NonViewParams,
    for<'w, 's> <T::Commands as GpuCommands>::ViewParams<'w, 's>: NonViewParams,
{
    fn run<'w>(
        &self, graph: &mut RenderGraphContext, context: &mut RenderContext<'w>, world: &'w World
    ) -> Result<(), NodeRunError> {
        <Self as ViewNode>::run(&self, graph, context, default(), world)
    }
}

impl<T: Pass + Compute> Node for ComputePassLabel<T> where
    for<'w, 's> <T::Binds as Bindings>::ViewParams<'w, 's>: NonViewParams,
    for<'w, 's> <T::Workgroups as WorkgroupArgs>::ViewParams<'w, 's>: NonViewParams,
    for<'w, 's> <T::Count as PassIter>::ViewParams<'w, 's>: NonViewParams,
    for<'w, 's> <T::Commands as GpuCommands>::ViewParams<'w, 's>: NonViewParams,
{
    fn run<'w>(
        &self, graph: &mut RenderGraphContext, context: &mut RenderContext<'w>, world: &'w World
    ) -> Result<(), NodeRunError> {
        <Self as ViewNode>::run(&self, graph, context, default(), world)
    }
}

impl<T: Pass + Raster> ViewNode for RasterPassLabel<T> {
    type ViewQuery = ViewRasterParams<'static, 'static, T>;

    fn run<'w>(
        &self, 
        _: &mut RenderGraphContext, 
        context: &mut RenderContext<'w>, 
        (v_bind, v_target, v_count, v_cmd): ViewRasterParams<'w, '_, T>, 
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
        let (w_bind, w_target, w_count, w_cmd) = system_state.get(world);
        let iterations = T::Count::iterations(w_count, v_count);

        let device = context.render_device();
        let bind_params = &mut get_bind_params(world);
        let layouts = &cached_pipeline.layouts;
        let Some(bind_group) = T::Binds::group(iterations, layouts, device, w_bind, v_bind, bind_params) else {
            return Ok(());
        };
        let Some(color_views) = T::Targets::get_views(iterations, w_target, v_target, bind_params) else {
            return Ok(());
        };

        let record = context.diagnostic_recorder();
        let commands = context.command_encoder();
        let time_span = record.time_span(commands, name);

        T::Commands::pre_iter(commands, w_cmd, v_cmd);

        for i in 0..iterations {

            let Some(color_attachments) = T::Targets::attachments(&color_views, i) else {
                error!("No ColorTarget attachments provided for {} at iteration {i}/{iterations}", name);
                continue;
            };
            let color_attachments = color_attachments.into_iter().map(|a| Some(a)).collect::<Vec<_>>();
            let color_attachments = color_attachments.as_slice();

            let mut render_pass = commands.begin_render_pass(&RenderPassDescriptor {
                label: Some(name),
                color_attachments,
                ..default()
            });

            render_pass.set_pipeline(pipeline);

            for g in 0..T::Binds::LEN {
                let bind_group = T::Binds::get_group(&bind_group, i, g);
                render_pass.set_bind_group(g, bind_group, &[]);
            }

            render_pass.draw(0..4, 0..1);
        }

        time_span.end(commands);
        Ok(())
    }
}

impl<T: Pass + Compute> ViewNode for ComputePassLabel<T> {
    type ViewQuery = ViewComputeParams<'static, 'static, T>;

    fn run<'w>(
        &self, 
        _: &mut RenderGraphContext, 
        context: &mut RenderContext<'w>, 
        (v_bind, v_wg, v_count, v_cmd): ViewComputeParams<'w, '_, T>, 
        world: &'w World,
    ) -> Result<(), NodeRunError> {

        let pipelines = world.resource::<PipelineCache>();
        let cached_pipeline = world.resource::<ComputePipeline<T>>();
        let Some(pipeline) = pipelines.get_compute_pipeline(cached_pipeline.id) else {
            warn!("Missing {}", type_name::<ComputePipeline::<T>>());
            return Ok(());
        };

        let name = type_name::<T>();

        let mut system_state = cached_pipeline.system_state.lock().unwrap();
        let (w_bind, w_wg, w_count, w_cmd) = system_state.get(world);
        let iterations = T::Count::iterations(w_count, v_count);

        let device = context.render_device();
        let bind_params = &mut get_bind_params(world);
        let layouts = &cached_pipeline.layouts;
        let Some(bind_group) = T::Binds::group(iterations, layouts, device, w_bind, v_bind, bind_params) else {
            return Ok(());
        };
        let UVec3 { x, y, z } = T::Workgroups::workgroups(w_wg, v_wg);

        let record = context.diagnostic_recorder();
        let commands = context.command_encoder();
        let time_span = record.time_span(commands, name);

        T::Commands::pre_iter(commands, w_cmd, v_cmd);

        for i in 0..iterations {

            let mut compute_pass = commands.begin_compute_pass(&ComputePassDescriptor {
                label: Some(name),
                ..default()
            });

            compute_pass.set_pipeline(pipeline);

            for g in 0..T::Binds::LEN {
                let bind_group = T::Binds::get_group(&bind_group, i, g);
                compute_pass.set_bind_group(g, bind_group, &[]);
            }

            compute_pass.dispatch_workgroups(x, y, z);
        }

        time_span.end(commands);
        Ok(())
    }
}
