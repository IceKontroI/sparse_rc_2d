use bevy::prelude::*;
use bevy::render::{render_asset::*, render_graph::*, render_resource::*, storage::*, texture::*};
use crate::gpu_api::{pass::*, utils::*};
use crate::gpu_resources::{slab::*, textures::*};

/// Doesn't do any work, just clears some resources each frame.
#[derive(Default, Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct Reset;

impl Pass for Reset {
    type Binds = ();
    type Count = Count<0>;
    type Commands = Self;
}

impl Compute for Reset {
    type Workgroups = WorkgroupDispatch<0, 0, 0>;
    const COMPUTE_SHADER_PATH: &'static str = "shaders/reset.wgsl";
}

impl GpuCommands for Reset {
    type WorldParams<'w, 's> = (
        Res<'w, RenderAssets<GpuShaderStorageBuffer>>,
        Res<'w, RenderAssets<GpuImage>>,
        Res<'w, Slabs>,
    );
    type ViewParams<'w, 's> = (
        &'w DirectLightingStorageB,
        &'w CoreBindGroup,
    );

    fn pre_iter(
        cmd: &mut CommandEncoder, 
        (buffers, images, slabs): Self::WorldParams<'_, '_>, 
        (direct_b, core): Self::ViewParams<'_, '_>
    ) {
        buffers.get(&core.statistics)
            .map(|buffer| cmd.clear_buffer(&buffer.buffer, 0, default()));
        buffers.get(&slabs.free)
            .map(|buffer| cmd.clear_buffer(&buffer.buffer, 0, default()));
        images.get(&direct_b.handle)
            .map(|image| cmd.clear_texture(&image.texture, &default()));
        images.get(&core.debug)
            .map(|image| cmd.clear_texture(&image.texture, &default()));
    }
}
