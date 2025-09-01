use std::{any::*, fmt::{Debug, Formatter}, hash::*, marker::*, ops::*};
use bevy::{ecs::{query::*, system::*}, prelude::*, render::view::ViewTarget};
use bevy::render::{render_asset::*, render_graph::*, render_resource::*, storage::*, texture::*};
use encase::internal::WriteInto;
use crate::color::ColorTarget;

use super::attach::Attach;

/// For binding Bevy ECS Components.
pub struct ViewBind<C: Component>(C);

/// For binding Bevy ECS Resources.
pub struct WorldBind<R: Resource>(R);

/// For cases where multiple images are part of the same bind group and only one should be the target.
pub struct FromAttach<C: Component + Attach<I>, const I: usize = 0>(C);

/// Enables the use of the command encoded before the first iteration occurs.
pub trait GpuCommands {
    type WorldParams<'w, 's>: for<'a, 'b> ReadOnlySystemParam<Item<'a, 'b> = Self::WorldParams<'a, 'b>>;
    type ViewParams<'w, 's>: for<'a, 'b> ReadOnlyQueryData<Item<'a, 'b> = Self::ViewParams<'a, 'b>>;

    fn pre_iter<'w, 's>(
        command_encoder: &mut CommandEncoder, 
        world_params: Self::WorldParams<'w, 's>,
        view_params: Self::ViewParams<'w, '_>,
    );
}

impl GpuCommands for () {
    type WorldParams<'w, 's> = ();
    type ViewParams<'w, 's> = ();
    
    fn pre_iter(_: &mut CommandEncoder, _: (), _: ()) {}
}

/// So the render pass can output number of iterations it should do.
pub trait PassIter {
    type WorldParams<'w, 's>: for<'a, 'b> ReadOnlySystemParam<Item<'a, 'b> = Self::WorldParams<'a, 'b>>;
    type ViewParams<'w, 's>: for<'a, 'b> ReadOnlyQueryData<Item<'a, 'b> = Self::ViewParams<'a, 'b>>;

    fn iterations<'w, 's>(world_params: Self::WorldParams<'w, 's>, view_params: Self::ViewParams<'w, '_>,) -> usize;
}

pub struct Count<const N: usize>;
impl<const N: usize> PassIter for Count<N> {
    type WorldParams<'w, 's> = ();
    type ViewParams<'w, 's> = ();

    fn iterations(_: (),  _: ()) -> usize { N }
}

/// Repeats a single value as many times as you like even if you go out of bounds.
/// Indexes into a Vec like normal, panicking when you are indexing out of bounds.
pub enum OOM<T> {
    One(T), Many(Vec<T>),
}
impl<T> Index<usize> for OOM<T> {
    type Output = T;
    fn index(&self, i: usize) -> &T {
        match self {
            Self::One(value) => &value,
            Self::Many(vec) => &vec[i],
        }
    }
}

/// Reusable struct to create a uniform of any object that implements ShaderType.
#[derive(AsBindGroup, Deref, DerefMut)]
pub struct Uniform<U: ShaderType + WriteInto> {
    #[uniform(0)]
    pub uniform: U,
}
impl<U: ShaderType + WriteInto> Uniform<U> {
    pub fn of(uniform: U) -> Self {
        Self { uniform }
    }
}

/// Color target for bevy's screen.
pub struct ViewColorTarget;
impl ColorTarget for ViewColorTarget {
    type WorldParams<'w, 's> = ();
    type ViewParams<'w, 's> = &'w ViewTarget;

    fn get_view(_: usize, _: (), view: &ViewTarget, _: &mut BindParams<'_>) -> Option<OOM<TextureView>> { 
        Some(OOM::One(view.post_process_write().destination.clone()))
    }
}

/// Lets us execute World (non-View) passes for anything with Empty in its ViewParams.
pub trait NonViewParams: Default {}
impl NonViewParams for () {}
impl NonViewParams for ((),) {}
impl NonViewParams for ((),(),) {}
impl NonViewParams for ((),(),(),) {}
impl NonViewParams for ((),(),(),(),) {}
impl NonViewParams for ((),(),(),(),(),) {}
impl NonViewParams for ((),(),(),(),(),(),) {}
impl NonViewParams for ((),(),(),(),(),(),(),) {}
impl NonViewParams for ((),(),(),(),(),(),(),(),) {}

/// Params requested by all of Bevy's AsBindGroup macro impls.
pub type BindParams<'w> = (
    Res<'w, RenderAssets<GpuImage>>, 
    Res<'w, FallbackImage>, 
    Res<'w, RenderAssets<GpuShaderStorageBuffer>>
);

/// Gets the params required for AsBindGroup for running a custom render pipeline pass.
/// Currently we're not able to get Res<R> from the World, which is the required format
/// so using transmute as a workaround.
/// 
/// TRACK the issue: https://github.com/bevyengine/bevy/issues/16831
pub fn get_bind_params<'w>(world: &'w World) -> BindParams<'w> {
    let gpu_images = world.resource_ref::<RenderAssets<GpuImage>>();
    let gpu_images: Res<RenderAssets<GpuImage>> = unsafe { std::mem::transmute(gpu_images) };
    let fallback_image = world.resource_ref::<FallbackImage>();
    let fallback_image: Res<FallbackImage> = unsafe { std::mem::transmute(fallback_image) };
    let gpu_ssbos = world.resource_ref::<RenderAssets<GpuShaderStorageBuffer>>();
    let gpu_ssbos: Res<RenderAssets<GpuShaderStorageBuffer>> = unsafe { std::mem::transmute(gpu_ssbos) };
    (gpu_images, fallback_image, gpu_ssbos)
}

// optionally you can use the RenderPass as a RenderLabel too
// but usually it's cleaner to just implement your own struct
// these macros won't work for T that doesn't impl the traits
// TODO this is messy and should be cleanly integrated into the bevy render graph

#[derive(RenderLabel)]
pub struct RasterPassLabel<T>(PhantomData<T>);
impl<T> Hash for RasterPassLabel<T> { fn hash<H: Hasher>(&self, state: &mut H) { self.0.hash(state); } }
impl<T> Debug for RasterPassLabel<T> { fn fmt(&self, f: &mut Formatter) -> std::fmt::Result { f.debug_tuple(type_name::<Self>()).field(&self.0).finish() } }
impl<T> Default for RasterPassLabel<T> { fn default() -> Self { Self(default()) } }
impl<T> Clone for RasterPassLabel<T> { fn clone(&self) -> Self { Self(self.0.clone()) } }
impl<T> PartialEq for RasterPassLabel<T> { fn eq(&self, other: &Self) -> bool { self.0 == other.0 } }
impl<T> Eq for RasterPassLabel<T> {}

#[derive(RenderLabel)]
pub struct ComputePassLabel<T>(PhantomData<T>);
impl<T> Hash for ComputePassLabel<T> { fn hash<H: Hasher>(&self, state: &mut H) { self.0.hash(state); } }
impl<T> Debug for ComputePassLabel<T> { fn fmt(&self, f: &mut Formatter) -> std::fmt::Result { f.debug_tuple(type_name::<Self>()).field(&self.0).finish() } }
impl<T> Default for ComputePassLabel<T> { fn default() -> Self { Self(default()) } }
impl<T> Clone for ComputePassLabel<T> { fn clone(&self) -> Self { Self(self.0.clone()) } }
impl<T> PartialEq for ComputePassLabel<T> { fn eq(&self, other: &Self) -> bool { self.0 == other.0 } }
impl<T> Eq for ComputePassLabel<T> {}
