use bevy::{app::*, asset::*, math::*, prelude::*};
use bevy::render::{extract_resource::*, render_resource::*, storage::*};
use crate::core::constants::*;
use crate::utils::extensions::*;

pub struct SlabPlugin;
impl Plugin for SlabPlugin {
    fn build(&self, app: &mut App) {
        app.init_extract_resource::<Slabs>();
    }
}

#[derive(Resource, Clone, ExtractResource, AsBindGroup)]
pub struct Slabs {
    #[storage(0, visibility(all))]
    pub task_slab: Handle<ShaderStorageBuffer>,
    #[storage(1, visibility(all))]
    pub color: Handle<ShaderStorageBuffer>,
    #[storage(2, visibility(all))]
    pub r: Handle<ShaderStorageBuffer>,
    #[storage(3, visibility(all))]
    pub free: Handle<ShaderStorageBuffer>,
}

impl FromWorld for Slabs {
    fn from_world(world: &mut World) -> Self {

        // this is the xy coordinate of the tasks, allocated as `array<vec2u, BANDWIDTH>` in wgsl
        let mut task_slab = ShaderStorageBuffer::from(vec![Vec2::default(); BANDWIDTH * SLAB_CAPACITY]);
        task_slab.buffer_description.usage = BufferUsages::STORAGE | BufferUsages::COPY_DST;
        task_slab.buffer_description.label = Some("Slab Tasks");

        // since we're using rgba8unorm, we can pack it into a single u32
        // since we're not using the alpha channel, and metadata only uses 8 bits, we can pack that in the alpha channel
        let mut color = ShaderStorageBuffer::from(vec![u32::default(); BANDWIDTH * SLAB_CAPACITY]);
        color.buffer_description.usage = BufferUsages::STORAGE | BufferUsages::COPY_DST;
        color.buffer_description.label = Some("Slab Color");

        // navigation array, pointing to the next (right)
        let mut r = ShaderStorageBuffer::from(vec![u32::default(); SLAB_CAPACITY]);
        r.buffer_description.usage = BufferUsages::STORAGE | BufferUsages::COPY_DST;
        r.buffer_description.label = Some("Slab Right");

        let mut free = ShaderStorageBuffer::from(0u32);
        free.buffer_description.usage = BufferUsages::STORAGE | BufferUsages::COPY_DST;
        free.buffer_description.label = Some("Slab Free");

        let mut buffers = world.resource_mut::<Assets<ShaderStorageBuffer>>();
        Slabs { 
            task_slab: buffers.add(task_slab),
            color: buffers.add(color),
            r: buffers.add(r),
            free: buffers.add(free),
        }
    }
}
