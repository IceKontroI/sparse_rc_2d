use std::f32::consts::TAU;
use bevy::{app::*, prelude::*};
use bevy::render::{extract_resource::*, render_resource::*};
use crate::core::{constants::*, math::*};
use crate::utils::extensions::*;

pub struct UniformsPlugin;
impl Plugin for UniformsPlugin {
    fn build(&self, app: &mut App) {
        app.init_extract_resource::<RcEnum>();
        app.init_extract_resource::<RcUniforms>();
        app.add_systems(PreUpdate, (
            update_rc_mode,
            update_function_mode,
            update_debug_mode,
            update_push_mode,
            update_params,
        ));
    }
}

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Resource, ExtractResource)]
pub enum RcEnum {
    SparseEdge = 0,
    #[default]
    SparseFilled = 1,
    Dense = 2,
}

fn update_rc_mode(
    mut rc_enum: ResMut<RcEnum>, 
    mut rcu: ResMut<RcUniforms>, 
    mut window: Single<&mut Window>,
    input: Res<ButtonInput<KeyCode>>,
) {
    if input.just_pressed(KeyCode::PageUp) {
        *rc_enum = match *rc_enum {
            RcEnum::Dense => RcEnum::SparseFilled,
            RcEnum::SparseFilled => RcEnum::SparseEdge,
            RcEnum::SparseEdge => return,
        };
    }
    if input.just_pressed(KeyCode::PageDown) {
        *rc_enum = match *rc_enum {
            RcEnum::SparseEdge => RcEnum::SparseFilled,
            RcEnum::SparseFilled => RcEnum::Dense,
            RcEnum::Dense => return,
        };
    }
    rcu.rc_model = *rc_enum as u32;
    window.title = format!("Radiance Cascades ({:?})", *rc_enum);
}

#[derive(Debug, Default, Copy, Clone, ShaderType)]
pub struct LevelParams {
    pub two_pow_index: u32,
    pub angle_ratio: f32,
    pub probe_spacing: u32,
    pub interval_start: u32,
}

#[derive(Debug, Default, Copy, Clone, Resource, ExtractResource, AsBindGroup)]
pub struct RcUniforms {
    // key controls
    #[uniform(0)] pub function_mode: u32,
    #[uniform(1)] pub debug_mode: u32,
    #[uniform(2)] pub push_mode: u32,
    #[uniform(3)] pub rc_model: u32,
    // core params
    #[uniform(4)] pub screen_dims: UVec2,
    #[uniform(5)] pub cascade_dims: UVec2,
    #[uniform(6)] pub num_cascades: u32,
    #[uniform(7)] pub texel_span: u32,
    // level params
    #[uniform(8)] pub cascade_level: u32,
    #[uniform(9)] pub level: [LevelParams; MAX_CASCADES],
}

fn update_function_mode(
    mut rcu: ResMut<RcUniforms>, 
    input: Res<ButtonInput<KeyCode>>,
) {
    let Some(new) = input.just_pressed_function() else {
        return
    };
    let old = rcu.function_mode;
    let new = new as u32;
    if old != new {
        rcu.function_mode = new;
        info!("Function Mode {old} -> {new}");
    } else if old  != 0 {
        rcu.function_mode = 0;
        info!("Function Mode {old} -> 0");
    }
}

fn update_debug_mode(
    mut rcu: ResMut<RcUniforms>, 
    input: Res<ButtonInput<KeyCode>>,
) {
    if let Some(new) = input.just_pressed_digit().or_else(|| {
        if input.just_pressed(KeyCode::Backquote) { Some(0) } else { None }
    }) { 
        let old = rcu.debug_mode;
        let new = new as u32;
        if old != new {
            rcu.debug_mode = new;
            info!("Debug Mode {old} -> {new}");
        }
    };
}

fn update_push_mode(
    mut rcu: ResMut<RcUniforms>, 
    input: Res<ButtonInput<KeyCode>>,
) {
    if !(input.just_pressed(KeyCode::Space) || input.just_released(KeyCode::Space)) {
        return;
    }
    let old = rcu.push_mode;
    let new = old ^ 1;
    info!("Push: {old} -> {new}");
    rcu.push_mode = new;
}

// TODO this can be mostly precomputed once on startup and then partially updated, but it's w/e
fn update_params(mut rcu: ResMut<RcUniforms>, camera: Single<&Camera>) {

    // update uniform screen_dims and other fields only if value is present and has changed
    rcu.screen_dims = match camera.physical_target_size() {
        Some(dims) if rcu.screen_dims != dims => dims,
        _ => return,
    };
    
    let interval_length = Vec2::ZERO.distance(UVec2::splat(PROBE_SPACING).as_vec2()) * 0.5;
    let c0_probe_spacing = ceil_to_power_of_n(PROBE_SPACING as f32, 2.0) as u32;
    let c0_interval_length = ceil_to_multiple_of_n(interval_length, 2.0) as u32;
    let Extent3d { width, height, depth_or_array_layers } = get_cascade_extents(rcu.screen_dims);
    rcu.cascade_dims = UVec2::new(width, height);
    rcu.num_cascades = depth_or_array_layers;
    rcu.texel_span = 1 << rcu.num_cascades;

    // we do num cascades + 1 so the last cascade can index into its theoretical parent
    for cascade_index in 0..(rcu.num_cascades + 1) {
        let two_pow_index = 1 << cascade_index;
        let four_pow_index = 1 << (2 * cascade_index);
        let angular_resolution = 4 * four_pow_index;
        rcu.level[cascade_index as usize] = LevelParams {
            two_pow_index,
            angle_ratio: TAU / angular_resolution as f32,
            probe_spacing: c0_probe_spacing * two_pow_index,
            interval_start: (c0_interval_length as i32 * (1 - four_pow_index as i32) / (1 - 4)) as u32,
        };
    }
}
