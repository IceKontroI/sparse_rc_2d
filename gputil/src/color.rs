use bevy::{ecs::{query::*, system::*}, prelude::*};
use bevy::render::render_resource::*;
use crate::utils::*;
use super::attach::*;

pub trait ColorTarget {
    type WorldParams<'w, 's>: for<'a, 'b> ReadOnlySystemParam<Item<'a, 'b> = Self::WorldParams<'a, 'b>>;
    type ViewParams<'w, 's>: for<'a, 'b> ReadOnlyQueryData<Item<'a, 'b> = Self::ViewParams<'a, 'b>>;

    const LOAD: bool = true;
    const STORE: bool = true;

    fn get_view<'w, 's>(
        iterations: usize, 
        world_params: Self::WorldParams<'w, 's>, 
        view_params: Self::ViewParams<'w, '_>,
        bind_params: &mut BindParams<'w>,
    ) -> Option<OOM<TextureView>>;

    fn attachment<'a>(_index: usize, texture_view: &'a TextureView) -> RenderPassColorAttachment<'a> {
        RenderPassColorAttachment { 
            view: texture_view,
            depth_slice: None,
            resolve_target: None,
            ops: Operations::<_> { 
                load: if Self::LOAD { LoadOp::Load } else { LoadOp::Clear(default()) }, 
                store: if Self::STORE { StoreOp::Store } else { StoreOp::Discard },
            }
        }
    }
}

/// Trait for specifying how to convert from instance -> TextureView -> RenderPassColorAttachment.
pub trait AsTextureView {
    fn as_texture_view(&self, bind_params: &mut BindParams<'_>) -> Option<TextureView>;
}

impl<C: Component + AsTextureView> ColorTarget for ViewBind<C> {
    type WorldParams<'w, 's> = ();
    type ViewParams<'w, 's> = &'w C;

    fn get_view(_: usize, _: (), view_params: &C, bind_params: &mut BindParams<'_>) -> Option<OOM<TextureView>> {
        Some(OOM::One(C::as_texture_view(view_params, bind_params)?))
    }            
}

impl<R: Resource + AsTextureView> ColorTarget for WorldBind<R> {
    type WorldParams<'w, 's> = Res<'w, R>;
    type ViewParams<'w, 's> = ();

    fn get_view(_: usize, world_params: Res<R>, _: (), bind_params: &mut BindParams<'_>) -> Option<OOM<TextureView>> {
        Some(OOM::One(R::as_texture_view(&world_params, bind_params)?))
    }
}

impl<const I: usize, C: Component + Attach<I>> ColorTarget for FromAttach<C, I> {
    type WorldParams<'w, 's> = ();
    type ViewParams<'w, 's> = &'w C;

    fn get_view<'w, 's>(_: usize, _: (), view_params: &C, bind_params: &mut BindParams<'w>) -> Option<OOM<TextureView>> {
        Some(OOM::One(bind_params.texture_view::<I>(view_params)?))
    }
}

// binding tuple types

pub trait ColorTargets {
    type WorldParams<'w, 's>: for<'a, 'b> ReadOnlySystemParam<Item<'a, 'b> = Self::WorldParams<'a, 'b>>;
    type ViewParams<'w, 's>: for<'a, 'b> ReadOnlyQueryData<Item<'a, 'b> = Self::ViewParams<'a, 'b>>;
    type Views;

    const LEN: u32;

    fn get_views<'w, 's>(
        iterations: usize, 
        world_params: Self::WorldParams<'w, 's>, 
        view_params: Self::ViewParams<'w, '_>,
        bind_params: &mut BindParams<'w>,
    ) -> Option<Self::Views>;

    fn attachments(views: &Self::Views, index: usize) -> Option<Vec<RenderPassColorAttachment>>;
}

impl ColorTargets for () {
    type WorldParams<'w, 's> = ();
    type ViewParams<'w, 's> = ();
    type Views = ();
    
    const LEN: u32 = 0;
                
    fn get_views(_: usize, _: (), _: (), _: &mut BindParams<'_>) -> Option<()> { Some(()) }
    fn attachments(_: &(), _: usize) -> Option<Vec<RenderPassColorAttachment>> { Some(vec![]) }
}

impl<A: ColorTarget> ColorTargets for A {
    type WorldParams<'w, 's> = A::WorldParams<'w, 's>;
    type ViewParams<'w, 's> = A::ViewParams<'w, 's>;
    type Views = OOM<TextureView>;
    
    const LEN: u32 = 1;
    
    fn get_views<'w, 's>(
        iterations: usize, 
        world_params: Self::WorldParams<'w, 's>, 
        view_params: Self::ViewParams<'w, '_>,
        bind_params: &mut BindParams<'w>,
    ) -> Option<Self::Views> {
        A::get_view(iterations, world_params, view_params, bind_params)
    }

    fn attachments(views: &Self::Views, index: usize) -> Option<Vec<RenderPassColorAttachment>> {
        Some(vec![A::attachment(index, &views[index]),])
    }
}

macro_rules! count {
    () => { 0 };
    ($head:tt $($tail:tt)*) => { 1 + count!($($tail)*) };
}

macro_rules! oom_view {
    ($_:ident) => { OOM<TextureView> };
}

macro_rules! impl_color_targets {
    ($($gen:ident $idx:tt)+) => {
        impl<$($gen: ColorTarget),+> ColorTargets for ($($gen,)+) {
            type WorldParams<'w, 's> = ($($gen::WorldParams<'w, 's>,)+);
            type ViewParams<'w, 's> = ($($gen::ViewParams<'w, 's>,)+);
            type Views = ($(oom_view!($gen),)+);

            const LEN: u32 = count!($($gen)+);

            fn get_views<'w, 's>(
                iterations: usize,
                world_params: Self::WorldParams<'w, 's>,
                view_params: Self::ViewParams<'w, '_>,
                bind_params: &mut BindParams<'w>,
            ) -> Option<Self::Views> {
                Some((
                    $($gen::get_view(
                        iterations, 
                        world_params.$idx, 
                        view_params.$idx, 
                        bind_params,
                    )?,)+
                ))
            }

            fn attachments(views: &Self::Views, index: usize) -> Option<Vec<RenderPassColorAttachment>> {
                Some(vec![
                    $($gen::attachment(index, &views.$idx[index]),)+
                ])
            }
        }
    };
}

impl_color_targets!(A 0);
impl_color_targets!(A 0 B 1);
impl_color_targets!(A 0 B 1 C 2);
impl_color_targets!(A 0 B 1 C 2 D 3);
impl_color_targets!(A 0 B 1 C 2 D 3 E 4);
impl_color_targets!(A 0 B 1 C 2 D 3 E 4 F 5);
impl_color_targets!(A 0 B 1 C 2 D 3 E 4 F 5 G 6);
impl_color_targets!(A 0 B 1 C 2 D 3 E 4 F 5 G 6 H 7);
