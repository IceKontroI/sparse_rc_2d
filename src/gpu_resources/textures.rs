use std::result::Result;
use bevy::{app::*, prelude::*, render::{render_resource::*, *}};
use extract_component::*;
use gpu_readback::*;
use ndex::*;
use chain_link::*;
use storage::*;
use crate::{core::math::*, debug::statistics::*, gpu_api::attach::*};

const ATTACHMENT_USAGES: TextureUsages = TextureUsages::RENDER_ATTACHMENT
    .union(TextureUsages::TEXTURE_BINDING)
    .union(TextureUsages::COPY_SRC)
    .union(TextureUsages::COPY_DST);

const STORAGE_USAGES: TextureUsages = TextureUsages::STORAGE_BINDING
    .union(TextureUsages::COPY_SRC)
    .union(TextureUsages::COPY_DST);

pub struct TexturesPlugin;
impl Plugin for TexturesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(AttachPlugin::<CoreBindGroup, AndExtract>::default());
        app.add_plugins(AttachPlugin::<JumpFloodA, AndExtract>::default());
        app.add_plugins(AttachPlugin::<JumpFloodB, AndExtract>::default());
        app.add_plugins(AttachPlugin::<DirectLightingA, AndExtract>::default());
        app.add_plugins(AttachPlugin::<DirectLightingB, AndExtract>::default());
        app.add_plugins(ExtractComponentPlugin::<DirectLightingStorageB>::default());
        app.add_systems(Startup, init_view_bindings);
        app.add_systems(Last, copy_lighting_handles);
    }
}

pub fn init_view_bindings(
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
    mut commands: Commands,
) {

    // Core bind group has the statistics readback added to it so we can limit all shaders to 
    // to 4 bind groups to maximize portability but that made this system a bit less readable
    let mut core_bind_group = CoreBindGroup::default();
    let mut buffer = ShaderStorageBuffer::from(Statistics::default());
    buffer.buffer_description.usage = BufferUsages::COPY_DST | BufferUsages::STORAGE | BufferUsages::COPY_SRC;
    buffer.buffer_description.label = Some("Statistics Readback Buffer");
    core_bind_group.statistics = buffers.add(buffer);
    commands.spawn(Readback::buffer(core_bind_group.statistics.clone()))
        .observe(readback); // readback system is at: `crate::debug::statistics::readback`

    commands.spawn((
        Projection::Orthographic(OrthographicProjection::default_2d()),
        Camera2d::default(),
        Camera::default(),
        Transform::default(),
        core_bind_group,
        JumpFloodA::default(),
        JumpFloodB::default(),
        DirectLightingA::default(),
        DirectLightingB::default(),
        DirectLightingStorageB::default(),
    ));
}

#[derive(Default, Clone, Index, IndexMut, Component, ExtractComponent, AsBindGroup)]
pub struct CoreBindGroup {
    #[index(0)]
    #[texture(0, filterable = false, visibility(all))]
    pub albedo: Handle<Image>,
    #[index(1)]
    #[texture(1, filterable = false, visibility(all))]
    pub emissive: Handle<Image>,
    #[index(2)]
    #[texture(2, filterable = false, visibility(all))]
    pub distance: Handle<Image>,
    #[index(3)]
    #[storage_texture(3, image_format = Rgba8Unorm, visibility(all))]
    pub debug: Handle<Image>,
    #[storage(4, visibility(all))]
    pub statistics: Handle<ShaderStorageBuffer>,
}
impl Attach<0> for CoreBindGroup {
    const TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;
    const TEXTURE_USAGES: TextureUsages = ATTACHMENT_USAGES;
    const COPY_ON_RESIZE: bool = true;
}
impl Attach<1> for CoreBindGroup {        
    const TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;
    const TEXTURE_USAGES: TextureUsages = ATTACHMENT_USAGES;
    const COPY_ON_RESIZE: bool = true;
}
impl Attach<2> for CoreBindGroup {
    const TEXTURE_FORMAT: TextureFormat = TextureFormat::R32Float;
    const TEXTURE_USAGES: TextureUsages = ATTACHMENT_USAGES;
    const COPY_ON_RESIZE: bool = true;
}
impl Attach<3> for CoreBindGroup {        
    const TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;
    const TEXTURE_USAGES: TextureUsages = STORAGE_USAGES;
    const COPY_ON_RESIZE: bool = true;

    // debug texture is in 1/4 resolution since it's debugging cascade-specific output
    // and because we're using pre-averaging cascade-level textures are in quarter-res
    fn compute_size(dimensions: UVec2) -> Extent3d {
        let Extent3d { width, height, .. } = get_cascade_extents(dimensions);
        Extent3d { width, height, depth_or_array_layers: 1 }
    }
}
impl Length for CoreBindGroup {
    type Len = L<4>;
}

/// Full-res texture used as the A side to ping pong and generate the unsigned distance field.
#[derive(Index, IndexMut, Component, Default, Clone, ExtractComponent, AsBindGroup)]
pub struct JumpFloodA {
    #[index(0)]
    #[texture(0, sample_type = "u_int", visibility(all))]
    handle: Handle<Image>,
}
impl Attach<0> for JumpFloodA {
    const TEXTURE_FORMAT: TextureFormat = TextureFormat::Rg32Uint;
    const TEXTURE_USAGES: TextureUsages = ATTACHMENT_USAGES;
    const COPY_ON_RESIZE: bool = true;
}
impl Length for JumpFloodA {
    type Len = L<1>;
}

/// Full-res texture used as the B side to ping pong and generate the unsigned distance field.
#[derive(Index, IndexMut, Component, Default, Clone, ExtractComponent, AsBindGroup)]
pub struct JumpFloodB {
    #[index(0)]
    #[texture(0, sample_type = "u_int", visibility(all))]
    handle: Handle<Image>,
}
impl Attach<0> for JumpFloodB {
    const TEXTURE_FORMAT: TextureFormat = TextureFormat::Rg32Uint;
    const TEXTURE_USAGES: TextureUsages = ATTACHMENT_USAGES;
    const COPY_ON_RESIZE: bool = true;
}
impl Length for JumpFloodB {
    type Len = L<1>;
}

#[derive(Index, IndexMut, Component, Default, Clone, ExtractComponent, AsBindGroup)]
pub struct DirectLightingA {
    #[index(0)]
    #[texture(0, filterable = false, visibility(all))]
    pub handle: Handle<Image>,
}
impl Length for DirectLightingA {
    type Len = L<1>;
}
impl Attach<0> for DirectLightingA {
    const TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;
    const TEXTURE_USAGES: TextureUsages = ATTACHMENT_USAGES;
    const COPY_ON_RESIZE: bool = true;

    fn compute_size(dimensions: UVec2) -> Extent3d {
        let Extent3d { width, height, .. } = get_cascade_extents(dimensions);
        Extent3d { width, height, depth_or_array_layers: 1 }
    }
}

#[derive(Index, IndexMut, Component, Default, Clone, ExtractComponent, AsBindGroup)]
pub struct DirectLightingB {
    #[index(0)]
    #[texture(0, filterable = false, visibility(all))]
    pub handle: Handle<Image>,
}
impl Length for DirectLightingB {
    type Len = L<1>;
}
impl Attach<0> for DirectLightingB {
    const TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;
    const TEXTURE_USAGES: TextureUsages = ATTACHMENT_USAGES.union(STORAGE_USAGES);
    const COPY_ON_RESIZE: bool = true;

    fn compute_size(dimensions: UVec2) -> Extent3d {
        let Extent3d { width, height, .. } = get_cascade_extents(dimensions);
        Extent3d { width, height, depth_or_array_layers: 1 }
    }
}

#[derive(Index, IndexMut, Component, Default, Clone, ExtractComponent, AsBindGroup)]
pub struct DirectLightingStorageB {
    #[index(0)]
    #[storage_texture(0, dimension = "2d", image_format = Rgba8Unorm, access = ReadWrite, visibility(all))]
    pub handle: Handle<Image>,
}

/// It's not straightforward to have a resource bound as both texture and storage_texture.
/// This maintains a new bind group resource with the same handle as the original so we can easily have both.
pub fn copy_lighting_handles(
    direct_lighting: Single<(&DirectLightingB, &mut DirectLightingStorageB)>,
) {
    let (texture, mut storage_texture) = direct_lighting.into_inner();
    storage_texture.handle = texture.handle.clone();
}
