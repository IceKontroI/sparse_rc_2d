use bevy::log::Level;

pub const LOG_LEVEL: Level = Level::INFO;

/// Using anything other than `2` will probably break stuff.
pub const PROBE_SPACING: u32 = 2;

/// Safe limit for cascade level which should be impossible to reach.
pub const MAX_CASCADES: usize = 32;

/// For sparse mode, this is the size of a slab AND the number of threads in a workgroup.
/// We keep them the same to maximize SIMD and memory read/write coalesce properties on the GPU.
/// 256 may be the maximum supported workgroup threads on some GPUs.
/// 256 also seemed to outperforms both 128 and 512 in some test scenes.
/// Higher values will have more idle threads on average but higher throughput potential.
pub const BANDWIDTH: usize = 256;

/// Number of slabs to allocate upfront for the life of the program.
/// Allocating too few slabs results in flickering of the lighting.
pub const SLAB_CAPACITY: usize = 64_000;

/// Size of a slab is
/// * 2x u32 for the xy coordinate of the task
/// * 1x u32 for the rgb of the light and the metadata
/// * 1x u32 for the `r` buffer to navigate to the next slab
pub const BYTES_PER_SLAB: usize = std::mem::size_of::<u32>() * 4;
