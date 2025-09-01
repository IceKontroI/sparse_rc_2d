use std::f32::consts::TAU;
use bevy::{app::*, input::mouse::*, prelude::*};
use bevy::render::{extract_resource::*, render_resource::*};
use rand::random;
use crate::core::{constants::*, math::*};
use crate::utils::extensions::*;

const COLORS: &[Vec4] = &[
    Vec4::new(0.025, 0.011, 0.18 , 1.0), // indigo
    Vec4::new(0.123, 0.01,  0.014, 1.0), // maroon
    Vec4::new(0.03,  0.05,  0.08 , 1.0), // slate gray
    Vec4::new(0.0,   0.1,   0.25 , 1.0), // sky
    Vec4::new(0.04,  0.2,   0.13 , 1.0), // forest
    Vec4::new(0.78,  0.85,  1.0  , 1.0), // fluorescent
    Vec4::new(0.9,   0.7,   0.4  , 1.0), // sun
    Vec4::new(0.05,  0.0,   0.47 , 1.0), // uv
    Vec4::new(0.82,  0.09,  0.09 , 1.0), // amaranth
    Vec4::new(0.735, 0.854, 0.424, 1.0), // lime
    Vec4::new(0.07,  0.48,  0.47 , 1.0), // turquoise
    Vec4::new(0.68,  0.21,  0.08 , 1.0), // flame
];

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
            update_mouse_data,
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
    // mouse drawing related
    pub mouse_color_index: usize,
    #[uniform(4)] pub mouse_brush_rgba: Vec4,
    #[uniform(5)] pub mouse_brush_size: f32,
    #[uniform(6)] pub mouse_button_pressed: u32,
    #[uniform(7)] pub mouse_last_pos: Vec2,
    #[uniform(8)] pub mouse_this_pos: Vec2,
    // core params
    #[uniform(9)] pub screen_dims: UVec2,
    #[uniform(10)] pub cascade_dims: UVec2,
    #[uniform(11)] pub num_cascades: u32,
    #[uniform(12)] pub texel_span: u32,
    // level params
    #[uniform(13)] pub cascade_level: u32,
    #[uniform(14)] pub level: [LevelParams; MAX_CASCADES],
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

fn update_mouse_data(
    mut rcu: ResMut<RcUniforms>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut mouse_moved: EventReader<CursorMoved>,
    mut mouse_wheel: EventReader<MouseWheel>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {

    if rcu.mouse_brush_size == 0.0 {
        // on program start, brush size will default to 0 so we correct it
        // but also take the opportunity to randomize the starting color
        rcu.mouse_brush_size = STARTING_BRUSH_SIZE;
        rcu.mouse_color_index = random::<u32>() as usize;
    } else if keyboard.just_pressed(KeyCode::Tab) {
        rcu.mouse_color_index += 1;
    }
    rcu.mouse_brush_rgba = COLORS[rcu.mouse_color_index % COLORS.len()];

    // update mouse button pressing status
    rcu.mouse_button_pressed = if mouse.just_pressed(MouseButton::Left) {
        1 // first press
    } else if mouse.pressed(MouseButton::Left) {
        2 // connected press
    } else {
        0 // not pressing
    };

    // update the last/this mouse position for us to interpolate between
    rcu.mouse_last_pos = rcu.mouse_this_pos;
    if let Some(moved) = mouse_moved.read().last() {
        rcu.mouse_this_pos = moved.position;
    };

    // update brush radius with scroll button
    let wheel_delta = mouse_wheel.read().map(|wheel| wheel.x + wheel.y).sum::<f32>();
    rcu.mouse_brush_size = f32::clamp(rcu.mouse_brush_size + wheel_delta, 1.0, 64.0);
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
            // interval lengths: [1, 7, 31, 127, 511, 2047]
            interval_start: (c0_interval_length as i32 * (1 - four_pow_index as i32) / (1 - 4)) as u32,
        };
    }
}
