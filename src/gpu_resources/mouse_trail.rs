use bevy::{input::mouse::*, prelude::*};
use bevy::render::extract_resource::*;
use rand::random;

// Colors generated with: https://apps.colorjs.io/picker/srgb-linear

/// Anything below 1 starts to have light leaks.
const MIN_BRUSH_SIZE: f32 = 2.0;

/// Colors that will be drawn as solids when the left mouse button is being pressed.
const ALBEDO_RGB: &[Vec3] = &[
    Vec3::new(0.025, 0.011, 0.18 ), // indigo
    Vec3::new(0.123, 0.01,  0.014), // maroon
    Vec3::new(0.03,  0.05,  0.08 ), // slate gray
    Vec3::new(0.0,   0.1,   0.25 ), // sky
    Vec3::new(0.04,  0.2,   0.13 ), // forest
];

/// Colors that will be drawn as emissive solids when the right mouse button is being pressed.
const EMISSIVE_RGB: &[Vec3] = &[
    Vec3::new(0.78,  0.85,  1.0  ), // fluorescent
    Vec3::new(0.9,   0.7,   0.4  ), // sun
    Vec3::new(0.05,  0.0,   0.47 ), // uv
    Vec3::new(0.82,  0.09,  0.09 ), // amaranth
    Vec3::new(0.735, 0.854, 0.424), // lime
    Vec3::new(0.07,  0.48,  0.47 ), // turquoise
    Vec3::new(0.68,  0.21,  0.08 ), // flame
];

pub struct MouseTrailPlugin;
impl Plugin for MouseTrailPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MouseTrail>();
        app.insert_resource(MouseTrail { radius: 8.0, ..default() });
        app.add_plugins(ExtractResourcePlugin::<MouseTrail>::default());
        app.add_systems(Update, mouse_drawing_system);
    }
}

/// Resource for tracking mouse trail and submitting quads to vertex shader.
#[derive(Default, Debug, Clone, ExtractResource, Resource)]
pub struct MouseTrail {
    pub radius: f32,
    pub last_quad: Option<[Vec4; 4]>, // Quad vertices in NDC
    pub is_drawing: bool,
    pub last_pos: Option<Vec2>,   // Position in pixel coordinates
    pub last_left: Option<Vec2>,  // Left edge in pixel coordinates
    pub last_right: Option<Vec2>, // Right edge in pixel coordinates
    pub connected: bool,
    pub brush: u32,
    pub smoothed_direction: Option<Vec2>, // Smoothed direction for smoother trails
    pub rgb: Vec3, // RGB color of the brush
}

pub enum BrushType {
    Opaque = 0, Emissive = 1,
}

pub fn mouse_drawing_system(
    camera: Single<&Camera>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut mouse_moved: EventReader<CursorMoved>,
    mut mouse_wheel: EventReader<MouseWheel>,
    mut mouse_trail: ResMut<MouseTrail>,
) {

    // Get camera dimensions
    let Some(UVec2 { x: width, y: height }) = camera.physical_target_size() else {
        return;
    };

    // Adjust brush radius with mouse wheel
    let wheel_delta: f32 = mouse_wheel.read().map(|wheel| wheel.x + wheel.y).sum();
    mouse_trail.radius = f32::max(MIN_BRUSH_SIZE, mouse_trail.radius + wheel_delta);

    // Select random color based on brush type when mouse is just pressed
    if mouse.just_pressed(MouseButton::Left) {
        let index = (random::<f32>() * ALBEDO_RGB.len() as f32).floor() as usize;
        mouse_trail.rgb = ALBEDO_RGB[index];
    } else if mouse.just_pressed(MouseButton::Right) {
        let index = (random::<f32>() * EMISSIVE_RGB.len() as f32).floor() as usize;
        mouse_trail.rgb = EMISSIVE_RGB[index];
    }

    // Handle mouse button input
    if mouse.pressed(MouseButton::Left) {
        mouse_trail.brush = BrushType::Opaque as u32;
    } else if mouse.pressed(MouseButton::Right) {
        mouse_trail.brush = BrushType::Emissive as u32;
    } else {
        mouse_trail.is_drawing = false;
        mouse_trail.last_pos = None;
        mouse_trail.last_quad = None;
        mouse_trail.last_left = None;
        mouse_trail.last_right = None;
        mouse_trail.connected = false;
        mouse_trail.smoothed_direction = None;
        return;
    }

    // Get latest mouse position
    let xy = match mouse_moved.read().last() {
        Some(last_move) => last_move.position,
        None => {
            mouse_trail.last_quad = None;
            return;
        }
    };

    // Convert pixel coordinates to NDC
    let to_ndc = |p: Vec2| -> Vec2 {
        Vec2 {
            x: 2.0 * (p.x / width as f32) - 1.0,
            y: 1.0 - 2.0 * (p.y / height as f32),
        }
    };

    if let Some(prev_pos) = mouse_trail.last_pos {
        mouse_trail.connected = true;
        let direction_pixels = xy - prev_pos;
        if direction_pixels.length_squared() > 0.0 {
            let current_direction = direction_pixels.normalize();
            let smoothed_direction = mouse_trail.smoothed_direction.map_or(
                current_direction,
                |prev| (prev * 0.5 + current_direction * 0.5).normalize()
            );
            mouse_trail.smoothed_direction = Some(smoothed_direction);
            let perp_pixels = Vec2::new(-smoothed_direction.y, smoothed_direction.x) * mouse_trail.radius;
            let current_left = xy + perp_pixels;
            let current_right = xy - perp_pixels;
            let last_left_val = mouse_trail.last_left.unwrap_or(prev_pos + perp_pixels);
            let last_right_val = mouse_trail.last_right.unwrap_or(prev_pos - perp_pixels);

            // Determine if we need to swap vertices to connect to the front-most edge
            let prev_direction = mouse_trail.smoothed_direction.unwrap_or(current_direction);
            let dot_product = current_direction.dot(prev_direction);
            let (connect_left, connect_right) = if dot_product < 0.0 {
                // Sharp turn detected, swap connections to avoid twisting
                (last_right_val, last_left_val)
            } else {
                (last_left_val, last_right_val)
            };

            mouse_trail.last_quad = Some([
                to_ndc(connect_left).extend(0.0).extend(1.0),  // Connect to previous left or right
                to_ndc(connect_right).extend(0.0).extend(1.0), // Connect to previous right or left
                to_ndc(current_left).extend(0.0).extend(1.0),  // Current left
                to_ndc(current_right).extend(0.0).extend(1.0), // Current right
            ]);
            mouse_trail.last_left = Some(current_left);
            mouse_trail.last_right = Some(current_right);
            mouse_trail.is_drawing = true;
        } else {
            mouse_trail.last_quad = None;
        }
    } else {
        // Initial position: create a small square
        let r = mouse_trail.radius;
        mouse_trail.last_quad = Some([
                to_ndc(xy + Vec2::new(-r, -r)).extend(0.0).extend(1.0),
                to_ndc(xy + Vec2::new( r, -r)).extend(0.0).extend(1.0),
                to_ndc(xy + Vec2::new(-r,  r)).extend(0.0).extend(1.0),
                to_ndc(xy + Vec2::new( r,  r)).extend(0.0).extend(1.0),
            ]);
            mouse_trail.connected = false;
            mouse_trail.is_drawing = false;
        mouse_trail.smoothed_direction = None;
    }

    mouse_trail.last_pos = Some(xy);
}
