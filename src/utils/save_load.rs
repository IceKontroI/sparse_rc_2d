use std::{any::*, mem::*, ops::*, path::*};
use bevy::{ecs::component::*, log::*, prelude::{Component, *}};
use bevy::render::{gpu_readback::*, render_resource::*};
use image::*;
use crate::utils::extensions::*;
use crate::gpu_resources::textures::*;

pub struct SaveLoadPlugin;
impl Plugin for SaveLoadPlugin {
    fn build(&self, app: &mut App) {
        app.spawn_single::<LockAlbedo>();
        app.spawn_single::<LockEmissive>();
        app.add_systems(Last, save_to_working_dir);
        app.add_systems(Last, load_from_working_dir);
        app.add_systems(Last, load_from_dragged_file);
    }
}

pub trait SaveImage: Deref<Target = bool> + DerefMut + Component<Mutability = Mutable> {
    const NAME: &str;
    const INDEX: usize;
}

#[derive(Default, Component, Deref, DerefMut)]
pub struct LockAlbedo(bool);
impl SaveImage for LockAlbedo {
    const NAME: &str = "albedo.png";
    const INDEX: usize = 0;
}

#[derive(Default, Component, Deref, DerefMut)]
pub struct LockEmissive(bool);
impl SaveImage for LockEmissive {
    const NAME: &str = "emissive.png";
    const INDEX: usize = 1;
}

pub fn save_to_working_dir(
    albedo: Single<(Entity, &mut LockAlbedo)>,
    emissive: Single<(Entity, &mut LockEmissive)>,
    scene: Single<&CoreBindGroup>,
    input: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
) {
    
    let (e_albedo, mut save_albedo) = albedo.into_inner();
    let (e_emissive, mut save_emissive) = emissive.into_inner();

    if input.just_control_pressed(KeyCode::KeyS) {
        commands.entity(e_albedo)
            .insert(Readback::Texture(scene[0].clone()))
            .observe(save_image::<LockAlbedo>);
        **save_albedo = false;

        commands.entity(e_emissive)
            .insert(Readback::Texture(scene[1].clone()))
            .observe(save_image::<LockEmissive>);
        **save_emissive = false;
    }
}

pub fn save_image<T: SaveImage>(
    mut trigger: On<ReadbackComplete>,
    single: Single<(Entity, &mut T)>,
    scene: Single<&CoreBindGroup>,
    images: Res<Assets<Image>>,
    mut commands: Commands,
) {

    // As long as the Readback component is on this Entity
    // it will attempt to read back each frame, so it gets
    // removed on trigger. But due to ECS delays, it still
    // attempts to read back for a few frames, but that is
    // fixed by the below locking mechanism.
    let (entity, mut lock) = single.into_inner();
    commands.entity(entity).remove::<Readback>();
    if !**lock {
        **lock = true;
    } else { return; }

    let Some(img) = images.get(&scene[T::INDEX]) else { return; };
    let UVec2 { x: width, y: height } = img.size();

    let readback = trigger.event_mut();
    let bytes: Vec<u8> = take(&mut readback.0);
    let image: ImageBuffer<Rgba<u8>, _> = ImageBuffer::from_raw(width, height, bytes).unwrap();

    let mut path = get_dir();
    path.push(T::NAME);
    match image.save(&path) {
        Ok(_) => info!("✅ Saved {} to {:?}", T::NAME, path),
        Err(e) => error!("❌ Failed to save {} to {:?}: {}", T::NAME, path, e),
    }
}

pub fn get_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn load_bytes_and_size<P: AsRef<Path>>(path: P) -> Option<(Vec<u8>, UVec2)> {
    match ImageReader::open(&path) {
        Ok(reader) => match reader.decode() {
            Ok(dyn_img) => {
                let rgba = dyn_img.to_rgba8();
                let bytes = rgba.into_raw();
                let size = UVec2::new(dyn_img.width(), dyn_img.height());
                info!("✅ Loaded image from {:?}", path.as_ref());
                Some((bytes, size))
            }
            Err(e) => {
                error!("❌ Failed to decode image {:?}: {}", path.as_ref(), e);
                None
            }
        },
        Err(e) => {
            error!("❌ Failed to open image {:?}: {}", path.as_ref(), e);
            None
        }
    }
}

pub fn load_from_working_dir(
    input: Res<ButtonInput<KeyCode>>,
    scene: Query<&CoreBindGroup>,
    mut images: ResMut<Assets<Image>>,
    mut window: Single<&mut Window>,
) {

    let Ok(scene) = scene.single() else {
        warn!("Missing {}", type_name::<CoreBindGroup>());
        return; 
    };

    if input.just_control_pressed(KeyCode::KeyL) {

        let mut albedo_path = get_dir();
        albedo_path.push("albedo.png");    
        let albedo_loaded = load_bytes_and_size(albedo_path);
        let albedo_image = images.get_mut(&scene[0]);
        if let (Some((loaded, UVec2 { x: width, y: height })), Some(image)) = (albedo_loaded, albedo_image) {
            image.data = Some(loaded);
            image.resize(Extent3d { width, height, depth_or_array_layers: 1 });
            window.resolution.set(width as f32, height as f32);
        }

        let mut emissive_path = get_dir();
        emissive_path.push("emissive.png");
        let emissive_loaded = load_bytes_and_size(emissive_path);
        let emissive_image = images.get_mut(&scene[1]);
        if let (Some((loaded, UVec2 { x: width, y: height })), Some(image)) = (emissive_loaded, emissive_image) {
            image.data = Some(loaded);
            image.resize(Extent3d { width, height, depth_or_array_layers: 1 });
            window.resolution.set(width as f32, height as f32);
        }
    }
}

pub fn load_from_dragged_file(
    mut events: EventReader<FileDragAndDrop>,
    mut images: ResMut<Assets<Image>>,
    scene: Single<&CoreBindGroup>,
    mut window: Single<&mut Window>,
) {
    let scene = scene.into_inner();

    for event in events.read() {
        if let FileDragAndDrop::DroppedFile { path_buf, .. } = event {
            let filename_lower = path_buf
                .file_name()
                .and_then(|f| f.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();

            if filename_lower.contains("albedo") {
                if let Some((bytes, size)) = load_bytes_and_size(path_buf) {
                    if let Some(image) = images.get_mut(&scene[0]) {
                        image.data = Some(bytes);
                        image.resize(Extent3d {
                            width: size.x,
                            height: size.y,
                            depth_or_array_layers: 1,
                        });
                        window.resolution.set(size.x as f32, size.y as f32);
                    }
                }
            } else if filename_lower.contains("emissive") {
                if let Some((bytes, size)) = load_bytes_and_size(path_buf) {
                    if let Some(image) = images.get_mut(&scene[1]) {
                        image.data = Some(bytes);
                        image.resize(Extent3d {
                            width: size.x,
                            height: size.y,
                            depth_or_array_layers: 1,
                        });
                        window.resolution.set(size.x as f32, size.y as f32);
                    }
                }
            } else {
                warn!("❌ Dropped file {:?} does not match albedo or emissive", path_buf);
            }
        }
    }
}
