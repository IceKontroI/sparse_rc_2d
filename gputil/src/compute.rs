use std::{any::*, marker::*, ops::*, sync::*};
use bevy::{ecs::{query::*, system::*}, prelude::*, shader::*};
use bevy::render::{diagnostic::*, render_asset::*, render_graph::{Node, *}, render_resource::*, renderer::*, storage::*};
use crate::{bind::*, utils::*};

// TODO add support for deferred compute args, custom draw commands, and other cool stuff
// TODO raster pass only supports triangle strip quads which is very limiting
// TODO set_bind_group(..., &[]); must support offsets instead of `&[]` for view binds: https://bevyengine.org/examples/shaders/custom-post-processing/
// TODO `depth_stencil_attachment: None` add support for depth/stencil attachments
// TODO pipelines use Arc<Mutex<SystemState>> which will break change detection in some cases, but this low priority

pub trait Compute: Sized + Send + Sync + 'static {
    type Binds: Bindings;
    type Count: PassIter;
    type Commands: GpuCommands;
    type Dispatch: ComputeDispatch;
    const COMPUTE_SHADER_PATH: &'static str;

    fn shader_defs() -> Vec<ShaderDefVal> { vec![] }
}

// TODO replace the other shit with this good shit
pub trait ComputeDispatch {
    type WorldParams<'w, 's>: for<'a, 'b> ReadOnlySystemParam<Item<'a, 'b> = Self::WorldParams<'a, 'b>>;
    type ViewParams<'w, 's>: for<'a, 'b> ReadOnlyQueryData<Item<'a, 'b> = Self::ViewParams<'a, 'b>>;

    fn get_dispatch_type<'w, 's>(
        world_params: Self::WorldParams<'w, 's>, 
        view_params: Self::ViewParams<'w, '_>,
    ) -> Option<ComputeDispatchType>;
}

pub enum ComputeDispatchType {
    Fixed(UVec3), 
    Indirect { 
        indirect_buffer: Buffer,
        indirect_offset: BufferAddress,
     },
}

pub struct StaticDispatch<const X: u32, const Y: u32 = 1, const Z: u32 = 1>;
impl<const X: u32, const Y: u32, const Z: u32> ComputeDispatch for StaticDispatch<X, Y, Z> {
    type WorldParams<'w, 's> = ();
    type ViewParams<'w, 's> = ();

    fn get_dispatch_type<'w, 's>(_: (), _: ()) -> Option<ComputeDispatchType> {
        Some(ComputeDispatchType::Fixed(UVec3 { x: X, y: Y, z: Z }))
    }
}

pub struct DeferredComputeArgs<R: Resource + Deref<Target = Handle<ShaderStorageBuffer>>>(R);
impl<R: Resource + Deref<Target = Handle<ShaderStorageBuffer>>> ComputeDispatch for DeferredComputeArgs<R> {
    type WorldParams<'w, 's> = (
        Res<'w, RenderAssets<GpuShaderStorageBuffer>>,
        Res<'w, R>,
    );
    type ViewParams<'w, 's> = ();

    fn get_dispatch_type(
        (buffers, args): Self::WorldParams<'_, '_>, _: ()
    ) -> Option<ComputeDispatchType> {
        let compute_args = buffers.get(&**args)?;
        let indirect_buffer = compute_args.buffer.clone();
        Some(ComputeDispatchType::Indirect { indirect_buffer, indirect_offset: 0 })
    }
}

pub type WorldComputeParams<'w, 's, T> = (
    <<T as Compute>::Binds as Bindings>::WorldParams<'w, 's>,
    <<T as Compute>::Dispatch as ComputeDispatch>::WorldParams<'w, 's>,
    <<T as Compute>::Count as PassIter>::WorldParams<'w, 's>,
    <<T as Compute>::Commands as GpuCommands>::WorldParams<'w, 's>,
);

pub type ViewComputeParams<'w, 's, T> = (
    <<T as Compute>::Binds as Bindings>::ViewParams<'w, 's>,
    <<T as Compute>::Dispatch as ComputeDispatch>::ViewParams<'w, 's>,
    <<T as Compute>::Count as PassIter>::ViewParams<'w, 's>,
    <<T as Compute>::Commands as GpuCommands>::ViewParams<'w, 's>,
);

#[derive(Resource)]
pub struct ComputePipeline<T: Compute> {
    layouts: <T::Binds as Bindings>::Layout,
    system_state: Arc<Mutex<SystemState<WorldComputeParams<'static, 'static, T>>>>,
    id: CachedComputePipelineId,
}

impl<T: Compute> FromWorld for ComputePipeline<T> {
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

impl<T: Compute> Node for ComputePassLabel<T> where
    for<'w, 's> <T::Binds as Bindings>::ViewParams<'w, 's>: NonViewParams,
    for<'w, 's> <T::Dispatch as ComputeDispatch>::ViewParams<'w, 's>: NonViewParams,
    for<'w, 's> <T::Count as PassIter>::ViewParams<'w, 's>: NonViewParams,
    for<'w, 's> <T::Commands as GpuCommands>::ViewParams<'w, 's>: NonViewParams,
{
    fn run<'w>(
        &self, graph: &mut RenderGraphContext, context: &mut RenderContext<'w>, world: &'w World
    ) -> Result<(), NodeRunError> {
        <Self as ViewNode>::run(&self, graph, context, default(), world)
    }
}

impl<T: Compute> ViewNode for ComputePassLabel<T> {
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
        let Some(dispatch) = T::Dispatch::get_dispatch_type(w_wg, v_wg) else {
            return Ok(());
        };

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

            match &dispatch {
                ComputeDispatchType::Fixed(UVec3 { x, y, z }) => {
                    compute_pass.dispatch_workgroups(*x, *y, *z);
                },
                ComputeDispatchType::Indirect { indirect_buffer, indirect_offset } => {
                    compute_pass.dispatch_workgroups_indirect(&indirect_buffer, *indirect_offset);
                },
                
            }
        }

        time_span.end(commands);
        Ok(())
    }
}
