/// Constants and math used widely throughout the program.
pub mod core {
    pub mod constants;
    pub mod math;
}

/// Render pass timestamps and readback statistics.
pub mod debug {
    pub mod metrics;
    pub mod statistics;
    pub mod timings;
}

/// Each of these rust modules (except `plugin`) corresponds to a WGSL shader.
/// The rc.wgsl shader is a common API that gets imported into a few of these.
pub mod gpu_passes {
    pub use self::{dist_field::*, dist_jfa_loop::*, dist_jfa_seed::*, draw::*, output::*, rc_dense::*, rc_sparse::*, reset::*};
    pub mod dist_field;
    pub mod dist_jfa_loop;
    pub mod dist_jfa_seed;
    pub mod draw;
    pub mod output;
    pub mod rc_dense;
    pub mod rc_sparse;
    pub mod reset;
    pub mod plugin;
}

/// High-level resources used in GPU rendering.
pub mod gpu_resources {
    pub mod mouse_trail;
    pub mod slab;
    pub mod textures;
    pub mod uniforms;
}

/// Not sure where else to put this stuff.
pub mod utils {
    pub mod extensions;
    pub mod launch;
    pub mod save_load;
}
