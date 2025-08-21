use std::{marker::*, ops::*};
use bevy::render::{extract_component::*, render_asset::*, render_resource::*, texture::*, view::*};
use bevy::{app::*, asset::*, ecs::component::*, image::*, math::*, prelude::*};
use chain_link::*;
use derive_builder::*;
use crate::gpu_api::utils::BindParams;

// TODO this whole thing could use a rework, and might be worth contributing to Bevy source directly

// TODO need a better way to auto-extract extractable attachable images that prevents user error
#[derive(Default)]
pub struct AndExtract;

type AttachParams<'a, T> = (&'a mut Assets<Image>, &'a mut T, UVec2);

#[derive(Default)]
pub struct AttachPlugin<A, E = ()>(PhantomData<(A, E)>);

impl<A> Plugin for AttachPlugin<A, ()>
where 
    A: Component<Mutability = Mutable>,
    for<'a> AttachPlugin::<A, ()>: Cascade<In<'a> = AttachParams<'a, A>>,
{
    fn build(&self, app: &mut App) {
        // // TODO PostUpdate seems like the only schedule that doesn't cause runtime WGPU errors
        // app.add_systems(PostUpdate, fix_bugs_system::<A>.before(resize_cascade_system::<A>));
        app.add_systems(PostUpdate, resize_cascade_system::<A>); // note cascade terminology is unrelated to Radiance Cascades
    }
}

impl<A: ExtractComponent> Plugin for AttachPlugin<A, AndExtract>
where 
    AttachPlugin<A, ()>: Default + Plugin,
{
    fn build(&self, app: &mut App) {
        app.add_plugins(AttachPlugin::<A, ()>::default());
        app.add_plugins(ExtractComponentPlugin::<A>::default());

    }
}

impl<A: Length, E> Length for AttachPlugin<A, E> {
    type Len = A::Len;
}

impl<const N: usize, A: Attach<N>, E> Chain<N> for AttachPlugin<A, E>
where 
    Self: InRange<N, Self::Len>,
{
    type In<'a> = AttachParams<'a, A>;
    type Out<'a> = AttachParams<'a, A>;

    fn chain((images, attach, physical_target_size): Self::In<'_>) -> Self::Out<'_> {

        let handle = &mut attach[N];
        let new_size = A::compute_size(physical_target_size);
        if new_size.width == 0 || new_size.height == 0 {
            // exit when minimized because dimensions become 0x0
            return (images, attach, physical_target_size)
        }

        // TODO why is it possible for the default handle to be a valid asset?
        if let Handle::Uuid(AssetId::<Image>::DEFAULT_UUID | AssetId::<Image>::INVALID_UUID, ..) = handle {
            warn!("Detected questionable (default?) handle, creating new image");
            *handle = images.add(A::new_image(new_size));
        } else if images.get(&*handle).is_none() {
            debug!("Edge case: possibly valid handle, but no image found, creating new one");
            *handle = images.add(A::new_image(new_size));
        }

        if let Some(image) = images.get(&*handle) {
            if image.texture_descriptor.size != new_size {
                // PR https://github.com/bevyengine/bevy/pull/19462
                if A::COPY_ON_RESIZE {
                    debug!("Copy-on-resize -> {physical_target_size:?}");
                    images.get_mut(handle).unwrap().resize_in_place(new_size);
                } else {
                    debug!("Default resize -> {physical_target_size:?}");
                    images.get_mut(handle).unwrap().texture_descriptor.size = new_size; // TODO breaks for data: Some(..)?
                }
            }
        }
        return (images, attach, physical_target_size)
    }
}

/// System to trigger a chain-link cascade through all of T's Attach<#> impls.
/// Iterates from 0..=N, sequentially resizing each defined Attach<#> type.
fn resize_cascade_system<A>(
    mut query: Query<(&mut A, &Camera)>, 
    mut images: ResMut<Assets<Image>>
) where
    A: Component<Mutability = Mutable>,
    for<'a> AttachPlugin::<A, ()>: Cascade<In<'a> = AttachParams<'a, A>>
{
    for (mut attach, camera) in &mut query {
        camera.physical_target_size()
            .map(|size| AttachPlugin::<A, ()>::cascade((&mut images, &mut attach, size)));
    }
}

pub trait Attach<const N: usize>
where
    Self: InRange<N, <Self as Length>::Len>,
    Self: Component<Mutability = Mutable>,
    Self: Index<usize, Output = Handle<Image>>,
    Self: IndexMut<usize>,
{
    const LABEL: Option<&'static str> = None;
    const BLEND_STATE: Option<BlendState> = None;
    const COLOR_WRITES: ColorWrites = ColorWrites::ALL;
    const TEXTURE_ASPECT: TextureAspect = TextureAspect::All;
    const COPY_ON_RESIZE: bool = false;
    const TEXTURE_FORMAT: TextureFormat;
    const TEXTURE_USAGES: TextureUsages;

    fn compute_size(UVec2 { x: width, y: height }: UVec2) -> Extent3d {
        Extent3d { width, height, depth_or_array_layers: 1 }
    }

    fn new_image(size: Extent3d) -> Image {
        Image {
            data: None,
            texture_descriptor: TextureDescriptor {
                label: Self::LABEL,
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: Self::TEXTURE_FORMAT,
                usage: Self::TEXTURE_USAGES,
                view_formats: &[],
            },
            texture_view_descriptor: Some(Self::texture_view(size).descriptor()),
            ..default()
        }
    }

    fn texture_view(size: Extent3d) -> ImageViewBuilder<'static> {
        ImageViewBuilder::default()
            .label(Self::LABEL)
            .format(Some(Self::TEXTURE_FORMAT))
            .dimension(Some(match size.depth_or_array_layers {
                0 => panic!("Cannot have 0 `depth_or_array_layers`"),
                1 => TextureViewDimension::D2,
                _ => TextureViewDimension::D2Array,
            }))
            .usage(Some(Self::TEXTURE_USAGES))
            .aspect(Self::TEXTURE_ASPECT)
            .base_mip_level(0)
            .mip_level_count(None)
            .base_array_layer(0)
            .array_layer_count(None)
    }
}

#[derive(Builder, Default)]
#[builder(default, pattern = "owned")] 
pub struct ImageView<'a> {
    pub label: Option<&'a str>,
    pub format: Option<TextureFormat>,
    pub dimension: Option<TextureViewDimension>,
    pub usage: Option<TextureUsages>,
    pub aspect: TextureAspect,
    pub base_mip_level: u32,
    pub mip_level_count: Option<u32>,
    pub base_array_layer: u32,
    pub array_layer_count: Option<u32>,
}

impl<'a> ImageViewBuilder<'a> {
    pub fn descriptor(self) -> TextureViewDescriptor<'a> {
        let builder = self.build().unwrap();
        TextureViewDescriptor {
            label: builder.label,
            format: builder.format,
            dimension: builder.dimension,
            usage: builder.usage,
            aspect: builder.aspect,
            base_mip_level: builder.base_mip_level,
            mip_level_count: builder.mip_level_count,
            base_array_layer: builder.base_array_layer,
            array_layer_count: builder.array_layer_count,
        }
    }
}

pub trait GetTextureView<A> {

    fn texture_view_fn<const N: usize, F>(&self, attach: &A, f: F) -> Option<TextureView> 
    where 
        A: Attach<N>, 
        F: FnOnce(ImageViewBuilder) -> ImageViewBuilder;

    fn texture_view<const N: usize>(&self, attach: &A) -> Option<TextureView> where A: Attach<N> {
        return self.texture_view_fn(attach, |b| {b});
    }
}
impl<A> GetTextureView<A> for RenderAssets<GpuImage> {
    fn texture_view_fn<const N: usize, F>(&self, attach: &A, f: F) -> Option<TextureView> 
    where 
        A: Attach<N>,
        F: FnOnce(ImageViewBuilder) -> ImageViewBuilder
    {
        let gpu_image = self.get(&attach[N])?;
        let mut builder = A::texture_view(gpu_image.size);
        builder = f(builder);
        let descriptor = builder.descriptor();
        let view = gpu_image.texture.create_view(&descriptor);
        Some(view)
    }
}
impl<'a, A> GetTextureView<A> for BindParams<'a> {
    fn texture_view_fn<const N: usize, F>(&self, attach: &A, f: F) -> Option<TextureView> 
    where 
        A: Attach<N>,
        F: FnOnce(ImageViewBuilder) -> ImageViewBuilder
    {
        self.0.texture_view_fn::<N, F>(attach, f)
    }
}

pub trait GetColorTargetState {
    fn color_target_state<const N: usize>() -> ColorTargetState where Self: Attach<N> {
        ColorTargetState {
            format: Self::TEXTURE_FORMAT,
            blend: Self::BLEND_STATE,
            write_mask: Self::COLOR_WRITES,
        }
    }
}
impl<T> GetColorTargetState for T {}

pub trait GetColorAttachment {
    fn color_attachment(&self) -> RenderPassColorAttachment<'_>;
}
impl GetColorAttachment for TextureView {
    fn color_attachment(&self) -> RenderPassColorAttachment<'_> {
        RenderPassColorAttachment {
            view: self,
            depth_slice: None,
            resolve_target: None,
            // TODO operations are too hardcoded and should be flexible (but still ergonomic)
            ops: Operations {
                load: LoadOp::Load,
                store: StoreOp::Store
            },
        }
    }
}
impl GetColorAttachment for PostProcessWrite<'_> {
    fn color_attachment(&self) -> RenderPassColorAttachment<'_> {
        RenderPassColorAttachment {
            view: self.destination,
            depth_slice: None,
            resolve_target: None,
            ops: default(),
        }
    }
}
