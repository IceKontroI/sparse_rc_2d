use bevy::{app::*, log::*, prelude::*, window::*};
use crate::core::constants::LOG_LEVEL;

pub struct Launch<const W: u32, const H: u32>;
impl<const W: u32, const H: u32> Plugin for Launch<W, H> {
    fn build(&self, app: &mut App) {
        app.add_plugins(DefaultPlugins
            .set(LogPlugin { level: LOG_LEVEL, ..default() })
            .set(WindowPlugin {
                primary_window: Some(Window { 
                    title: "Radiance Cascades".into(),
                    position: WindowPosition::Centered(MonitorSelection::Primary),
                    resolution: WindowResolution::new(W, H)
                        .with_scale_factor_override(1.0),
                    ..default() 
                }
            ), ..default()
        }));
    }
}