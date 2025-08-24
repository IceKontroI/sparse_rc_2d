use bevy::ecs::{query::*, system::*};
use bevy::render::render_resource::*;
use crate::utils::*;

pub trait DepthTarget {
    type WorldParams<'w, 's>: for<'a, 'b> ReadOnlySystemParam<Item<'a, 'b> = Self::WorldParams<'a, 'b>>;
    type ViewParams<'w, 's>: for<'a, 'b> ReadOnlyQueryData<Item<'a, 'b> = Self::ViewParams<'a, 'b>>;

    const DEPTH_OPS: Option<Operations<f32>> = Some(Operations { load: LoadOp::Load, store: StoreOp::Store });
    const STENCIL_OPS: Option<Operations<u32>> = None;

    // TODO clarify structure: outer Option<OOM<...>> denotes success/fail
    // TODO and inner Option<TextureView> denotes whether the pass has an attachment for depth
    //      but they should either all have Some or all have None... this lets us do a mix which is why it's confusing
    fn get_view<'w, 's>(
        iterations: usize, 
        world_params: Self::WorldParams<'w, 's>, 
        view_params: Self::ViewParams<'w, '_>,
        bind_params: &mut BindParams<'w>,
    ) -> Option<OOM<Option<TextureView>>>;

    fn depth_attachment<'a>(views: &'a OOM<Option<TextureView>>, index: usize) -> Option<RenderPassDepthStencilAttachment<'a>> {
        views[index].as_ref().map(|view| RenderPassDepthStencilAttachment {
            view,
            depth_ops: Self::DEPTH_OPS,
            stencil_ops: Self::STENCIL_OPS,
        })
    }
}

impl DepthTarget for () {
    type WorldParams<'w, 's> = ();
    type ViewParams<'w, 's> = ();

    fn get_view(_: usize, _: (), _: (), _: &mut BindParams<'_>) -> Option<OOM<Option<TextureView>>> { 
        Some(OOM::One(None)) 
    }
}
