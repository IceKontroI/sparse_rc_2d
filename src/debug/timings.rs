use std::{any::*, marker::*};
use bevy::{app::*, diagnostic::*, log::*, prelude::*};
use bevy::render::diagnostic::*;
use pretty_type_name::*;
use crate::gpu_api::pass::*;
use crate::gpu_passes::*;
use super::metrics::*;

pub struct RenderPassTimingsPlugin;
impl Plugin for RenderPassTimingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RenderDiagnosticsPlugin);
        app.add_systems(Last, print_render_pass_timings);
    }
}

/// Prints GPU (not CPU) timings for all render passes.
/// Also aggregates these metrics into a frame-level metric.
pub fn print_render_pass_timings(
    d: Res<DiagnosticsStore>,
    mut reset: Metrics<Reset>,
    mut draw: Metrics<Draw>,
    mut dist_jfa_seed: Metrics<DistJfaSeed>,
    mut dist_jfa_loop: Metrics<DistJfaLoop>,
    mut dist_field: Metrics<DistField>,
    mut rc_dense: Metrics<RcDense>,
    mut rc_sparse: Metrics<RcSparse>,
    mut output: Metrics<Output>,
    mut frame: Metrics<Frame>,
) {
    let mut total = 0.0;
    total += apply_and_get_time(&d, &mut reset);
    total += apply_and_get_time(&d, &mut draw);
    total += apply_and_get_time(&d, &mut dist_jfa_seed);
    total += apply_and_get_time(&d, &mut dist_jfa_loop);
    total += apply_and_get_time(&d, &mut dist_field);
    total += apply_and_get_time(&d, &mut rc_dense);
    total += apply_and_get_time(&d, &mut rc_sparse);
    total += apply_and_get_time(&d, &mut output);
    frame += total;
}

pub fn get_path<T: Pass>() -> DiagnosticPath {
    DiagnosticPath::new(format!("render/{}/elapsed_gpu", type_name::<T>()))
}

pub fn apply_and_get_time<T: Pass>(diag: &DiagnosticsStore, metrics: &mut Metrics<T>) -> f64 {
    let time = diag.get(&get_path::<T>())
        // can also use `smoothed` or `average`
        .map(Diagnostic::value)
        .flatten().unwrap_or_default();
    *metrics += time;
    time
}

impl<T: Pass + Send + Sync + 'static> Metric for T {
    type Data = f64;
    
    fn emit(ms: f64, frames: u32) {
        let ms = ms / frames as f64;
        // if ms > 0.005 {
            let name = pretty_type_name::<T>();
            info!("{name}: {ms:.3} MS");
        // }
    }
}

pub struct Frame;
impl Metric for Frame {
    type Data = f64;
    
    fn emit(ms: f64, frames: u32) {
        let ms = ms / frames as f64;
        info!("Frame: {ms:.3} MS");
    }
}
