use std::ops::*;
use bevy::{diagnostic::*, ecs::system::*, prelude::*};

pub trait Metric: Send + Sync + 'static {
    const FRAME_INTERVAL: u32 = 160;
    type Data: Default + Send + Sync + 'static;
    
    fn emit(count: Self::Data, frames: u32);
}

#[derive(SystemParam)]
pub struct Metrics<'w, 's, M: Metric> {
    frame: Res<'w, FrameCount>,
    resource: Option<ResMut<'w, MetricsTracker<M>>>,
    commands: Commands<'w, 's>,
}

impl<'w, 's, T, M> AddAssign<T> for Metrics<'w, 's, M> where 
    M: Metric<Data: AddAssign<T>> + Send + Sync, 
    T: Send + Sync + 'static,
{
    fn add_assign(&mut self, rhs: T) {
        if let Some(res) = self.resource.as_deref_mut() {
            res.count += rhs;
            res.emit(self.frame.0);
        } else {
            // first metric triggers the creation of the metrics tracking resource
            self.commands.queue(move |world: &mut World| {
                let frame = world.resource::<FrameCount>().0;
                world.init_resource::<MetricsTracker<M>>();
                let mut res = world.resource_mut::<MetricsTracker<M>>();
                res.count += rhs;
                res.emit(frame);
            });
        }
    }
}

#[derive(Resource)]
struct MetricsTracker<M: Metric> {
    count: M::Data,
    frame: u32,
}

impl<M: Metric> FromWorld for MetricsTracker<M> {
    fn from_world(world: &mut World) -> Self {
        MetricsTracker::<M> {
            count: default(), 
            frame: world.resource::<FrameCount>().0, 
        }
    }
}

impl<M: Metric> MetricsTracker<M> {
    fn emit(&mut self, frame: u32) {
        if frame - self.frame >= M::FRAME_INTERVAL {
            let count = std::mem::take(&mut self.count);
            M::emit(count, frame - self.frame);
            self.count = default();
            self.frame = frame;
        }
    }
}
