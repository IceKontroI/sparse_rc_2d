use std::marker::*;
use bevy::{ecs::{query::*, system::*}, prelude::*};
use bevy::render::{render_resource::*, renderer::*};
use crate::utils::*;

pub struct BindContext<'a, 'w> {
    pub layout: &'a BindGroupLayout,
    pub device: &'a RenderDevice,
    pub bind_params: &'a mut BindParams<'w>,
}

/// Trait required for any parameters in a Pass's Bindings.
pub trait Bind {
    type WorldParams<'w, 's>: for<'a, 'b> ReadOnlySystemParam<Item<'a, 'b> = Self::WorldParams<'a, 'b>>;
    type ViewParams<'w, 's>: for<'a, 'b> ReadOnlyQueryData<Item<'a, 'b> = Self::ViewParams<'a, 'b>>;

    fn layout(device: &RenderDevice) -> BindGroupLayout;

    fn group<'w, 's>(
        iterations: usize, 
        world_params: Self::WorldParams<'w, 's>, 
        view_params: Self::ViewParams<'w, '_>,
        context: BindContext<'_, 'w>,
    ) -> Option<OOM<BindGroup>>;
}

impl<C: Component + AsBindGroup<Param = BindParams<'static>>> Bind for ViewBind<C> {
    type WorldParams<'w, 's> = ();
    type ViewParams<'w, 's> = &'w C;

    fn layout(device: &RenderDevice) -> BindGroupLayout {
        C::bind_group_layout(device)
    }

    fn group(_: usize, _: (), component: &C, c: BindContext) -> Option<OOM<BindGroup>> {
        Some(OOM::One(component.as_bind_group(c.layout, c.device, c.bind_params).ok()?.bind_group))
    }
}

impl<R: Resource + AsBindGroup<Param = BindParams<'static>>> Bind for WorldBind<R> {
    type WorldParams<'w, 's> = Res<'w, R>;
    type ViewParams<'w, 's> = ();
        
    fn layout(device: &RenderDevice) -> BindGroupLayout {
        R::bind_group_layout(device)
    }

    fn group(_: usize, resource: Res<R>, _: (), c: BindContext) -> Option<OOM<BindGroup>> {
        Some(OOM::One(resource.as_bind_group(c.layout, c.device, c.bind_params).ok()?.bind_group))
    }
}

pub trait Bindings {
    type WorldParams<'w, 's>: for<'a, 'b> ReadOnlySystemParam<Item<'a, 'b> = Self::WorldParams<'a, 'b>>;
    type ViewParams<'w, 's>: for<'a, 'b> ReadOnlyQueryData<Item<'a, 'b> = Self::ViewParams<'a, 'b>>;
    type Layout: Send + Sync;
    type Group;

    const LEN: u32;

    fn layout(device: &RenderDevice) -> Self::Layout;

    fn layout_vec(layout: &Self::Layout) -> Vec<BindGroupLayout>;

    fn group<'w, 's>(
        iterations: usize, 
        layout: &Self::Layout, 
        device: &RenderDevice,
        world_params: Self::WorldParams<'w, 's>, 
        view_params: Self::ViewParams<'w, '_>,
        bind_params: &mut BindParams<'w>,
    ) -> Option<Self::Group>;

    fn get_group(groups: &Self::Group, iteration: usize, group_number: u32) -> &BindGroup;
}

impl Bindings for () {
    type WorldParams<'w, 's> = ();
    type ViewParams<'w, 's> = ();
    type Layout = ();
    type Group = ();
    
    const LEN: u32 = 0;
                
    fn layout(_: &RenderDevice) {}
    fn layout_vec(_: &Self::Layout) -> Vec<BindGroupLayout> { vec![] }
    fn group(_: usize, _: &(), _: &RenderDevice, _: (), _: (), _: &mut BindParams<'_>) -> Option<()> { Some(()) }
    fn get_group(_: &Self::Group, _: usize, _: u32) -> &BindGroup { unreachable!() }
}

impl<A: Bind> Bindings for A {
    type WorldParams<'w, 's> = A::WorldParams<'w, 's>;
    type ViewParams<'w, 's> = A::ViewParams<'w, 's>;
    type Layout = BindGroupLayout;
    type Group = OOM<BindGroup>;
    
    const LEN: u32 = 1;
    
    fn layout(device: &RenderDevice) -> Self::Layout {
        A::layout(device)
    }

    fn layout_vec(layout: &Self::Layout) -> Vec<BindGroupLayout> { 
        vec![layout.clone()] 
    }
    
    fn group<'w, 's>(
        iterations: usize, 
        layout: &Self::Layout, 
        device: &RenderDevice,
        world_params: Self::WorldParams<'w, 's>, 
        view_params: Self::ViewParams<'w, '_>,
        bind_params: &mut BindParams<'w>,
    ) -> Option<Self::Group> {
        A::group(iterations, world_params, view_params, BindContext { layout: &layout, device, bind_params })
    }
    
    fn get_group(groups: &Self::Group, iteration: usize, group_number: u32) -> &BindGroup {
        match group_number {
            0 => &groups[iteration],
            _ => unreachable!()
        }
    }
}

macro_rules! count {
    () => { 0 };
    ($head:tt $($tail:tt)*) => { 1 + count!($($tail)*) };
}

macro_rules! bind_group_layout {
    ($_:ident) => { BindGroupLayout };
}

macro_rules! oom_bind_group {
    ($_:ident) => { OOM<BindGroup> };
}

macro_rules! impl_bindings {
    ($($gen:ident $idx:tt)+) => {
        impl<$($gen: Bind),+> Bindings for ($($gen,)+) {
            type WorldParams<'w, 's> = ($($gen::WorldParams<'w, 's>,)+);
            type ViewParams<'w, 's> = ($($gen::ViewParams<'w, 's>,)+);
            type Layout = ($(bind_group_layout!($gen),)+);
            type Group = ($(oom_bind_group!($gen),)+);

            const LEN: u32 = count!($($gen)+);

            fn layout(device: &RenderDevice) -> Self::Layout {
                ($($gen::layout(device),)+)
            }

            fn layout_vec(layout: &Self::Layout) -> Vec<BindGroupLayout> { 
                vec![$(layout.$idx.clone(),)+] 
            }
            
            fn group<'w, 's>(
                iterations: usize, 
                layout: &Self::Layout, 
                device: &RenderDevice,
                world_params: Self::WorldParams<'w, 's>, 
                view_params: Self::ViewParams<'w, '_>,
                bind_params: &mut BindParams<'w>,
            ) -> Option<Self::Group> {
                Some((
                    $($gen::group(
                        iterations, 
                        world_params.$idx, 
                        view_params.$idx, 
                        BindContext { 
                            layout: &layout.$idx, 
                            device, bind_params,
                        },
                    )?,)+
                ))
            }

            fn get_group(groups: &Self::Group, iteration: usize, group_number: u32) -> &BindGroup {
                match group_number {
                    $($idx => &groups.$idx[iteration],)+
                    _ => unreachable!()
                }
            }
        }
    };
}

impl_bindings!(A 0);
impl_bindings!(A 0 B 1);
impl_bindings!(A 0 B 1 C 2);
impl_bindings!(A 0 B 1 C 2 D 3);
impl_bindings!(A 0 B 1 C 2 D 3 E 4);
impl_bindings!(A 0 B 1 C 2 D 3 E 4 F 5);
impl_bindings!(A 0 B 1 C 2 D 3 E 4 F 5 G 6);
impl_bindings!(A 0 B 1 C 2 D 3 E 4 F 5 G 6 H 7);