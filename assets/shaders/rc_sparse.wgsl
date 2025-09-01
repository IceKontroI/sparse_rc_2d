#import "shaders/rc.wgsl" as rc

// CRITICAL threads can be severely underutilized (as low as 60% utilization rate in some scenes)
// Invocations == slab length so if a slab is only partially populated, excess threads do nothing
// This happens on the last slab of a chain, and is quadrupled due to directional chain iteration
// This may explain why there was no performance difference upgrading `BANDWIDTH` from 256 -> 512
// This should be fixable but is a major shader rework and will delay Sparse implementation in 3D

// TODO There's a noticeable color difference in the Sparse model from the rgba8unorm compression
// Fixed by storing slabs in a texture instead of a buffer, but would limit allocation dimensions

const INVALID_TASK: vec2u = vec2u(4294967295u, 4294967295u);

const MAX_CASCADE: u32 = 32u;

/// This is simultaneously number of workgroup invocations and length of a Slab's data array.
/// Must be a multiple of the target hardware's subgroup size possibly 4, 16, 32, 64, or 128.
/// To maximize portability, stick to multiples of 128, as subgroup size is known at runtime.
const BANDWIDTH: u32 = #{BANDWIDTH};

/// Accounts for the max possible number of lanes, derived from the min `subgroup_size` of 4.
const MAX_LANES: u32 = BANDWIDTH / 4u;

/// Total slabs allocated upfront for the life of the program.
const SLAB_CAPACITY: u32 = #{SLAB_CAPACITY};

/// Amount of space to allocate for deduplication scratch array.
/// At most a child probe can have 3 siblings (it's the 4th).
/// So deduplication requires 3 comparisons in the worst case.
const DEDUPE_LEN: u32 = BANDWIDTH + 3u;

// [bindings]

@group(2) @binding(0)
var direct_lighting: texture_storage_2d<rgba8unorm, read_write>;

@group(3) @binding(0)
var<storage, read_write> task_slab: array<array<vec2u, BANDWIDTH>, SLAB_CAPACITY>;
@group(3) @binding(1)
var<storage, read_write> color: array<array<u32, BANDWIDTH>, SLAB_CAPACITY>;
@group(3) @binding(2)
var<storage, read_write> r: array<u32, SLAB_CAPACITY>;
@group(3) @binding(3)
var<storage, read_write> free: atomic<u32>;

// [compute]

/// The workgroup's number of lanes computed using the runtime `subgroup_size` compute param.
var<workgroup> total_lanes: atomic<u32>;
/// For sharing prefix sum data between lanes in the workgroup.
var<workgroup> prefix_sum: array<u32, MAX_LANES>;
/// The lane that was assigned to this thread's whole subgroup.
var<private> lane: u32;
/// Unique index within a lane for this thread.
var<private> lane_index: u32;
/// Unique thread index in the range [0, BANDWIDTH), sequential within lanes.
var<private> thread_index: u32;


/// Total number of reads/writes this iteration.
var<workgroup> count: atomic<u32>;
/// Total number of reads/writes for the current loop (not including `count`).
var<workgroup> len: u32;

var<workgroup> cascade_chains: array<Chain, MAX_CASCADE>;
struct Chain {
    head: u32,
    len: u32,
}

/// The slab to write to index into for variable read/write operations.
/// When casting rays, `this/next` slab is used for the variable write operation.
/// When merging rays, `this/next` slab is used for the variable read operation.
var<workgroup> this_slab: u32;
/// If `this_slab` is full, index into this slab, then set `this_slab` == `next_slab`.
var<workgroup> next_slab: u32;
/// The final slab in the currently allocated slab chain.
var<workgroup> tail_slab: u32;
/// False means we allocated all slabs and should stop writing.
var<workgroup> has_slab: bool;
/// Used in lazy slab allocation to avoid allocating for truly sparse cascade hierarchies.
var<workgroup> slab_init: bool;

var<workgroup> dedupe: array<vec2u, DEDUPE_LEN>;
var<private> dedupe_index: u32;

/// Cheap way of letting the merge section know what the starting cascade should be.
var<workgroup> merge_start: atomic<u32>;

@compute
@workgroup_size(BANDWIDTH, 1, 1) // subgroup ops only available for 1D workgroups
fn compute(
    @builtin(subgroup_invocation_id) s_id: u32,
    @builtin(subgroup_size) subgroup_size: u32,
    @builtin(workgroup_id) workgroup_id: vec3u,
) {
    groupInit(subgroup_size, s_id);
    if c0Seed(workgroup_id) && castRays() {
        mergeRays();
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// MACRO FUNCTIONS ////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

fn c0Seed(workgroup_id: vec3u) -> bool {

    // texel_span is the width of a cascade in texels
    // each workgroup processes one cascade hierarchy
    let hierarchy_xy = workgroup_id.xy * rc::texel_span;
    let texel_volume = rc::texel_span * rc::texel_span;
    let slab_count = slabCoverage(texel_volume);
    var edges = 0u;

    for (var i = 0u; (!slab_init || has_slab) && i < slab_count; i += 1u) {
        groupWriteStart(0u);

        let position = i * BANDWIDTH + thread_index;
        let texel = hierarchy_xy + zCurve(position);
        let in_bounds = isInBounds(i, texel_volume);
        let valid = in_bounds && c0TaskValid(texel);
        let task = select(INVALID_TASK, texel / 2u, valid);
        let wrote = groupWrite(task, 0u);
        edges += select(0u, 1u, wrote);

        groupWriteStop(0u);
    }

    edges = groupSum(edges);
    if edges != 0u && thread_index == 0u {
        atomicAdd(&rc::statistics.c0_tasks, edges);
    }
    return edges != 0u;
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// RAY CASTING ////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

fn castRays() -> bool {

    var merge_count = 0u;
    var ray_hits = 0u;

    for (var c = 0u; has_slab && c < rc::num_cascades; c += 1u) {
        let task_len = cascade_chains[c].len;
        let slab_len = slabCoverage(task_len);
        if slab_len == 0u { break; }
                
        if thread_index == 0u {
            slabNext(false);
            startChain(c + 1u);
        }

        let child_cascade = c + 1u < rc::num_cascades; // not the highest level cascade

        workgroupBarrier();

        for (var ray_dir = 0u; has_slab && ray_dir < 4u; ray_dir += 1u) {
            var read_slab = cascade_chains[c].head;
            for (var i = 0u; i < slab_len; i += 1u) {
                groupWriteStart(c + 1u);

                let in_bounds = isInBounds(i, task_len);
                let ray_task = task_slab[read_slab][thread_index];
                var merge_xy: vec2u;
                var is_merge = false;
                var m = 0u;

                workgroupBarrier();

                if ray_dir == 0u {
                    var rgb = vec3f(0.0, 0.0, 0.0);
                    if in_bounds {
                        let result = rc::completeTask(ray_task, c);
                        merge_xy = result[0u].merge_xy;
                        is_merge = result[0u].is_merge;
                        for (var d = 0u; d < 4u; d += 1u) {
                            ray_hits += select(0u, 1u, result[d].hit);
                            merge_count += select(0u, 1u, result[d].is_merge);
                            m |= select(0u, 1u, result[d].hit) << d;
                            rgb += result[d].direct;
                        }
                        atomicAdd(&rc::statistics.rays_per_level[c], 4u);
                        if rc::function_mode == rc::TASK_VISUALIZER && rc::debug_mode == u32(c) {
                            let task_data = vec4u(1u, m, 0u, 0u); // must rescale this to be rgba8unorm
                            textureStore(rc::debug_texture, vec2i(ray_task), vec4f(task_data) / 255.0);
                        }
                        if is_merge {
                            // start merging at the highest cascade level
                            atomicMax(&merge_start, c);
                        }
                    }
                    setColorAndMetadata(read_slab, thread_index, rgb * 0.25, m);
                } else {
                    let linear_resolution: vec2u = rc::cascade_dims / (1u << c);
                    let coord_within_block: vec2u = ray_task % linear_resolution;
                    let dir_block_index: vec2u = ray_task / linear_resolution;
                    let dir_index: u32 = (dir_block_index.x + dir_block_index.y * (1u << c)) * 4u;
                    let preavg_dir_index: u32 = dir_index + ray_dir;
                    m = getMetadata(read_slab, thread_index);
                    let non_occluded = ((m >> ray_dir) & 1u) == 0u;
                    merge_xy = rc::getMergeTexelAt(c, preavg_dir_index, coord_within_block);
                    is_merge = non_occluded && child_cascade;
                }

                workgroupBarrier();

                let merge_task = select(INVALID_TASK, merge_xy, in_bounds && is_merge);
                let unique_bit = groupWrite(merge_task, c + 1u);
                orMetadata(read_slab, thread_index, select(0u, 1u, unique_bit) << (4u + ray_dir));

                read_slab = r[read_slab];
                groupWriteStop(c + 1u);
            }
        }
    }

    ray_hits = groupSum(ray_hits);
    if thread_index == 0u {
        atomicAdd(&rc::statistics.ray_hits, ray_hits);
    }

    let total_merges = groupSum(merge_count);
    return total_merges > 0u;
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// MERGING ////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

fn mergeRays() {

    var merge_count = 0u;

    for (var c = i32(merge_start); c >= 0; c -= 1) {
        let task_len = cascade_chains[c].len;
        let slab_len = slabCoverage(task_len);
        if slab_len == 0u { continue; }

        this_slab = cascade_chains[c+1].head;
        next_slab = r[this_slab];
        len = 0u;
        count = 0u;

        workgroupBarrier();

        for (var ray_dir = 0u; ray_dir < 4u; ray_dir += 1u) {
            var child_slab = cascade_chains[c].head;
            for (var i = 0u; i < slab_len; i += 1u) {

                prefix_sum[lane] = 0u;
                let in_bounds = isInBounds(i, task_len);

                // TODO more detailed instructions for why prefix sum with `unique` condition is right index
                let m = getMetadata(child_slab, thread_index);
                let no_hit = ((m >> (     ray_dir)) & 1u) == 0u; // no hit, ray continues
                let unique = ((m >> (4u + ray_dir)) & 1u) != 0u; // unique = offset index

                // offset can be 0, so `read_index < start_index` will be true, making it select `next_slab`
                // but these cases are actually along the seam and must always read from `this_slab` instead
                let offset = groupPrefixSum(unique, true);
                let start_index = len % BANDWIDTH;
                let read_index = (BANDWIDTH + len + offset - 1u) % BANDWIDTH;
                let color_slab = select(this_slab, next_slab, offset != 0u && read_index < start_index);
                
                let actually_merge = in_bounds && no_hit;
                let mul = select(0.0, 0.25, actually_merge);
                var merge_color = getColor(color_slab, read_index) * mul;
                var current_color = getColor(child_slab, thread_index);
                current_color += merge_color;
                setColor(child_slab, thread_index, current_color);
                merge_count += select(0u, 1u, actually_merge);

                // block mode renders the cascade blocks on-screen at a specified cascade level, selected with debug_mode
                if rc::function_mode == rc::CASCADE_BLOCK_MODE && rc::debug_mode == u32(c) {
                    let xy = task_slab[child_slab][thread_index];
                    if actually_merge && groupUnique(xy) {
                        let rgb = getColor(child_slab, thread_index);
                        textureStore(rc::debug_texture, xy, vec4f(rgb, 1.0));
                    }
                }

                // last direction of c0 cascade applies lighting
                if in_bounds && c == 0 && ray_dir == 3u {
                    let task = task_slab[child_slab][thread_index];
                    let color = getColor(child_slab, thread_index);
                    textureStore(direct_lighting, task, vec4f(color.r, color.g, color.b, 1.0));
                }

                child_slab = r[child_slab];
                groupReadStop();
            }
        }
    }

    merge_count = groupSum(merge_count);
    if thread_index == 0u && merge_count > 0u {
        atomicAdd(&rc::statistics.merge_count, merge_count);
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// GROUP FUNCTIONS ////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// All functions with `group` prefix must be done in unison or they won't work.
/// Do not call these inside if-statements unless you can guarantee no thread divergence.
/// These have workgroupBarrier() built-in so manual barriers are usually not needed.

fn groupInit(subgroup_size: u32, subgroup_invocation_id: u32) {
    slab_init = false;
    lane_index = subgroup_invocation_id;
    if lane_index == 0u {
        // TRACK https://github.com/gfx-rs/wgpu/issues/5555
        // eventually atomics replaced with `num_subgroups`
        lane = atomicAdd(&total_lanes, 1u);
    }
    workgroupBarrier();
    lane = subgroupBroadcastFirst(lane);
    thread_index = (lane * subgroup_size) + lane_index;
    dedupe_index = thread_index + 3u;
    // else (0,0) c0 task is flagged as duplicate
    dedupe[thread_index] = INVALID_TASK;
}

fn groupWriteStart(c: u32) {
    len = cascade_chains[c].len;
    prefix_sum[lane] = 0u;
}

fn groupWrite(xy: vec2u, c: u32) -> bool {

    if thread_index == 0u {
        count = 0u;
    }
    let unique = groupUnique(xy);
    let sum = groupPrefixSum(unique, false);
    if lane_index == 0u {
        atomicAdd(&count, prefix_sum[lane]);
    }

    // lazy slab allocation on first write
    if !slab_init {
        workgroupBarrier();
        if thread_index == 0u && count != 0u {
            slab_init = true;
            slabNext(true);
            startChain(0u);
        }
        workgroupBarrier();
    }

    if unique {

        // store only unique tasks
        let index = (len + sum) % BANDWIDTH;
        let start_index = len % BANDWIDTH;
        let write_slab = select(this_slab, next_slab, index < start_index);
        task_slab[write_slab][index] = xy;

        // probe placement/duplication debugging
        let duplicate_mode = rc::function_mode == rc::PROBE_DUPLICATE_MODE;
        let placement_mode = rc::function_mode == rc::TASK_VISUALIZER;
        if rc::debug_mode == u32(c) && (duplicate_mode || placement_mode) {
            let r = 1.0 + (textureLoad(rc::debug_texture, xy).r * 255.0);
            textureStore(rc::debug_texture, xy, vec4f(r / 255.0, 0.0, 0.0, 0.0));
        }
    }

    return unique;
}

/// Deduplicates a task by checking the 3 previous tasks, returning whether that task is unique.
/// This works because tasks are initially iterated in z-curve order and inserted directionally.
/// Only 3 lookups are needed because children are contiguous and in the worst case, this thread
/// is the 4th child and the rays cast by all 4 children were not hits.
fn groupUnique(xy: vec2u) -> bool {
    dedupe[dedupe_index % DEDUPE_LEN] = xy;
    workgroupBarrier();
    let unique = any(xy != INVALID_TASK) 
        && any(xy != dedupe[(dedupe_index - 1u) % DEDUPE_LEN]) 
        && any(xy != dedupe[(dedupe_index - 2u) % DEDUPE_LEN]) 
        && any(xy != dedupe[(dedupe_index - 3u) % DEDUPE_LEN]);
    dedupe_index += DEDUPE_LEN - 3u;
    return unique;
}

/// Takes the in/exclusive prefix sum for the workgroup, counting frequency of `condition == true`.
fn groupPrefixSum(condition: bool, inclusive: bool) -> u32 {
    let add = select(0u, 1u, condition);
    var sum = subgroupInclusiveAdd(add);
    prefix_sum[lane] = subgroupMax(sum);
    workgroupBarrier();
    for (var a = 0u; a < lane; a += 1u) {
        sum += prefix_sum[a];
    }
    sum -= select(add, 0u, inclusive);
    return sum;
}

var<workgroup> workgroup_sum: atomic<u32>;

fn groupSum(thread_sum: u32) -> u32 {
    if thread_index == 0u {
        workgroup_sum = 0u;
    }
    workgroupBarrier();
    let subgroup_sum = subgroupAdd(thread_sum);
    if lane_index == 0u {
        atomicAdd(&workgroup_sum, subgroup_sum);
    }
    workgroupBarrier();
    return workgroup_sum;
}

fn groupWriteStop(ci_write: u32) {
    workgroupBarrier();
    if thread_index == 0u && count > 0u {
        // if we wrote to next_slab this iteration, this_slab is full
        // advance this_slab <- next_slab and allocate when necessary
        // to ensure that on the next iteration, `next_slab` is empty
        let old_index = cascade_chains[ci_write].len % BANDWIDTH;
        cascade_chains[ci_write].len += count;
        let new_index = cascade_chains[ci_write].len % BANDWIDTH;
        if new_index <= old_index {
            slabNext(false);
        }
    }
    workgroupBarrier();
}

fn groupReadStop() {
    workgroupBarrier();
    if lane_index == 0u {
        atomicAdd(&count, prefix_sum[lane]);
    }
    workgroupBarrier();
    if thread_index == 0u && count > 0u {
        let old_index = len % BANDWIDTH;
        len += atomicExchange(&count, 0u);
        let new_index = len % BANDWIDTH;
        if new_index <= old_index {
            this_slab = next_slab;
            next_slab = r[next_slab];
        }
    }
    workgroupBarrier();
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// SLAB FUNCTIONS /////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

// at first, min allocation is 2 since both `this_slab` and `next_slab` need slabs
// after that, allocate at least 1 so `next` doubles as the `tail` at the smallest
// increments `next_slab` if possible on subsequent calls, to skip over-allocation
fn slabNext(init: bool) {
    if !init && next_slab < tail_slab {
        this_slab = next_slab;
        next_slab += 1u;
    } else {
        let amount = select(2u, 2u, init);
        let head = atomicAdd(&free, amount);
        let tail = min(SLAB_CAPACITY, head + amount) - 1u;
        if head >= SLAB_CAPACITY || tail - head < select(0u, 1u, init) {
            atomicAdd(&rc::statistics.data_lost, 1u);
            has_slab = false;
            return;
        }
        atomicAdd(&rc::statistics.slabs_allocated, 1u + tail - head);
        this_slab = select(next_slab, head, init);
        next_slab = select(head, head + 1u, init);
        tail_slab = tail;      
    }
    r[this_slab] = next_slab;
    has_slab = true;
}

fn startChain(ci_write: u32) {
    cascade_chains[ci_write].head = this_slab;
    cascade_chains[ci_write].len = 0u;
}

fn getColor(slab: u32, index: u32) -> vec3f {
    return unpack4x8unorm(color[slab][index]).rgb;
}

fn setColor(slab: u32, index: u32, rgb: vec3f) {
    let rgb_packed = pack4x8unorm(vec4f(rgb, 0.0));
    color[slab][index] = insertBits(color[slab][index], rgb_packed, 0u, 24u);
}

fn getMetadata(slab: u32, index: u32) -> u32 {
    return extractBits(color[slab][index], 24u, 8u);
}

fn orMetadata(slab: u32, index: u32, bits: u32) {
    color[slab][index] |= bits << 24u;
}

fn setColorAndMetadata(slab: u32, index: u32, rgb: vec3f, metadata: u32) {
    let rgb_packed = pack4x8unorm(vec4f(rgb, 0.0));
    color[slab][index] = rgb_packed | (metadata << 24u);
}

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/// OTHER FUNCTIONS ////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

/// Number of slabs that would cover the amount of items, assuming a contiguous slab-chain.
fn slabCoverage(items: u32) -> u32 {
    return (items + (BANDWIDTH - 1u)) / BANDWIDTH;
}

fn isInBounds(i: u32, length: u32) -> bool {
    let position = i * BANDWIDTH + thread_index;
    let in_bounds = position < length;
    if in_bounds {
        atomicAdd(&rc::statistics.threads_active, 1u);
    } else {
        atomicAdd(&rc::statistics.threads_idle, 1u);
    }
    return in_bounds;
}

/// Spaces an integer so it can be zippered into the z-curve.
fn space(i: u32) -> u32 {
    var x = i;
    x = (x      >> 0 ) & 0x55555555u;
    x = (x | (x >> 1)) & 0x33333333u;
    x = (x | (x >> 2)) & 0x0F0F0F0Fu;
    x = (x | (x >> 4)) & 0x00FF00FFu;
    x = (x | (x >> 8)) & 0x0000FFFFu;
    return x;
}

/// Given a 1D index and the dimensions of the 2D area, return a 2D index in morton-order.
fn zCurve(linear: u32) -> vec2u {
    return vec2u(space(linear), space(linear >> 1));
    // // to verify the deduplication process, return this instead
    // return vec2u(
    //     linear % rc::texel_span, 
    //     linear / rc::texel_span,
    // );
}

const EMPTY: u32 = 0u;
const MIXED: u32 = 1u;
const SOLID: u32 = 2u;

fn sampleAlbedoQuad(xy: vec2i) -> u32 {
    var solid = 0u;
    for (var i = 0; i < rc::QUAD_OFFSETS_LEN; i += 1) {
        let sample = xy + rc::QUAD_OFFSETS[i];
        let alpha = rc::loadAlbedo(sample).a;
        solid += u32(ceil(alpha));
    }
    switch solid {
        case 0u: { return EMPTY; }
        case 4u: { return SOLID; }
        default: { return MIXED; }
    }
}

/// Returns true when some xy screen texel should have a c0 seed probe placed on it.
fn c0TaskValid(xy: vec2u) -> bool {
    if any(xy >= rc::screen_dims) {
        return false;
    }

    // sparse filled mode puts probes everywhere
    if rc::rc_model == 1u {
        return true;
    }

    // sparse edge mode puts probes only on the edges (2d) or surfaces (3d) of solids
    // IDEA alternative: complete c0 tasks here and store those with hits > 0 and < 4
    var empty = 0;
    for (var i = 0; i < rc::DIAG_OFFSETS_LEN; i += 1) {
        let sample = vec2i(xy) + rc::DIAG_OFFSETS[i];
        let result = sampleAlbedoQuad(sample);
        // this helps with lighting in tight spaces but adds more c0 tasks
        if result == MIXED { return true; }
        empty += select(0, 1, result == EMPTY);
    }
    return empty > 0 && empty < rc::DIAG_OFFSETS_LEN;
}
