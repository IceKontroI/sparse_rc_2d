use bevy::app::*;
use bevy::render::{render_graph::*, *};
use bevy::core_pipeline::core_2d::graph::*;
use gputil::{compute::*, raster::*, utils::*};
use super::*;

pub struct RenderPassesPlugin;
impl Plugin for RenderPassesPlugin {

    fn build(&self, app: &mut App) {

        let render_app = app.get_sub_app_mut(RenderApp)
            .expect("[BUILD] Missing RenderApp");

        render_app
            .add_render_graph_node::<ViewNodeRunner<ComputePassLabel<Reset>>>(Core2d, Reset)
            .add_render_graph_node::<ViewNodeRunner<RasterPassLabel<Draw>>>(Core2d, Draw)
            .add_render_graph_node::<ViewNodeRunner<RasterPassLabel<DistJfaSeed>>>(Core2d, DistJfaSeed)
            .add_render_graph_node::<ViewNodeRunner<RasterPassLabel<DistJfaLoop>>>(Core2d, DistJfaLoop)
            .add_render_graph_node::<ViewNodeRunner<RasterPassLabel<DistField>>>(Core2d, DistField)
            .add_render_graph_node::<ViewNodeRunner<RasterPassLabel<RcDense>>>(Core2d, RcDense)
            .add_render_graph_node::<ViewNodeRunner<ComputePassLabel<RcSparse>>>(Core2d, RcSparse)
            .add_render_graph_node::<ViewNodeRunner<RasterPassLabel<Output>>>(Core2d, Output);

        render_app.add_render_graph_edges(Core2d, (
            Node2d::StartMainPass,
            Node2d::Tonemapping,
            Reset,
            Draw,
            DistJfaSeed,
            DistJfaLoop,
            DistField,
            RcDense,
            RcSparse,
            Output,
            Node2d::EndMainPassPostProcessing,
        ));
    }

    fn finish(&self, app: &mut App) {

        let render_app = app.get_sub_app_mut(RenderApp)
            .expect("[FINISH] Missing RenderApp");

        render_app
            .init_resource::<ComputePipeline<Reset>>()
            .init_resource::<RasterPipeline<Draw>>()
            .init_resource::<RasterPipeline<DistJfaSeed>>()
            .init_resource::<RasterPipeline<DistJfaLoop>>()
            .init_resource::<RasterPipeline<DistField>>()
            .init_resource::<RasterPipeline<RcDense>>()
            .init_resource::<ComputePipeline<RcSparse>>()
            .init_resource::<RasterPipeline<Output>>();
    }
}
