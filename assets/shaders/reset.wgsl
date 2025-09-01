#import "shaders/rc.wgsl" as rc

@compute @workgroup_size(1, 1, 1)
fn compute() {
    // rc::ray_deferred_args.vertex_count = 2u;
    rc::ray_deferred_args.instance_count = 0u;
    // rc::ray_deferred_args.first_vertex = 0u;
    // rc::ray_deferred_args.first_instance = 0u;
}