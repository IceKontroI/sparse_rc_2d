# Overview

This is a 2D test program, and work will soon begin extending it to 3D to finalize the concept. The primary motivation is to reduce the memory storage requirements of Radiance Cascades in 3D to make it more viable in production. For more information on the design of Sparse RC, see [the public google slides](https://docs.google.com/presentation/d/1shDaU-fOrhVIg4lGkITJcxVnf-VkVVTahlxngPiqeMs/edit?usp=sharing) where anyone with that link can leave a comment, or email me at `icekontroi@gmail.com`.

This uses Bevy for its ECS, asset management, and graph-based GPU pipeline. All render passes are made from scratch. A custom Bevy-WGPU wrapper API is used to minimize boilerplate and keep business logic tidy.

The Sparse and Dense models all have:
- 2x linear scaling
- 4x angular scaling
- nearest-neighbor fix
- pre-averaging optimization
- the same raymarching and merge logic
- zero interpolation

---
# Toggle SparseEdge/SparseFilled/Dense

This app showcases 3 mode of RC. By default, the program initialized with SparseFilled, which places c0 probes at all non-solid positions in the scene. Effectively this stores lighting in the "air" of the scene. It's capable of generate an output 1:1 with the Dense (conventional) RC model.

SparseEdge attempts to put c0 probes on the edges of solids in the scene, although it's not perfect. A better approach would be to actually ray-cast all c0 probes of the scene and store only the probes where the number of rays that hit something is > 0 (in empty space) and < 4 (not fully enclosed in a solid). This could be a future optimization to explore. While edge lighting may not be desireable in 2D, in 3D it becomes surface lighting, which is exactly what you want (unless you want to do volumetrics).

Both SparseFilled and SparseEdge mode leverage the same sparse model. The Dense model is just conventional RC.

Use `PageUp` to go from:
- SparseFilled to SparseEdge
- Dense to SparseFilled

Use `PageDown` to go from
- SparseEdge to SparseFilled
- SparseFilled to Dense

The setting won't "wrap" so spamming page up will not pass SparseEdge, and likewise spamming PageDown will stop at Dense.

---
# Scene Drawing/Saving/Loading

Press left mouse button to draw solids and right button to draw lights. Currently erasing the scene is not supported. Colors are chosen at random.

Scenes have an Albedo and Emissive layer. Sample scenes are found in `../assets/scenes/`.
- Drag and drop images onto the app window to load them. Only .png with "albedo" and "emissive" in their names are accepted.
- Use `ctrl + s` to save the current scene to a local directory.
- Use `ctrl + l` to load whatever was last saved.

Note there is a bug where saved images are wrapped incorrectly if the screen's window was manually resized. And there is an aesthetic bug where saved scene files are darker than they are drawn in the application due to color space normalization.

---
# Debug Modes

Pressing the same function key twice reverts to F0 render mode which is just normal RC output.

F1/F2/F3/F4 all rely on a selected cascade level to generate output. Use the digit keys (0-9) to select the cascade level to visualize.

- **F1: Task Visualizer** mode shows which tasks were generated for the sparse models and how many times the task was occluded (hit).
- **F2: Task Deduplication** mode shows missing tasks as black, non-duplicated tasks as blue, and duplicates as red.
- **F3: Cascade Block** mode debugs the lighting stored in individual cascades. It allows you to directly view the texture of all cascades at a particular level on-screen.
- **F4: Cascade Interval** mode lights the scene using only the lighting from the selected cascade level, to better visualize the intervals generated at that cascade level.
- **F5: Distance Field** mode shows the actual (unsigned) distance field used in raymarching.

---
# Caveats

Various dense structures are used, even in the sparse model:
- The distance field texture and JFA textures are all dense and are used to accelerate ray-casting. Each 3D implementation will have its own app-specific acceleration structures, so no effort was made to make this part of the process sparse. And keeping it the same between Dense and Sparse models makes their performance directly comparable.
- The Sparse model uses a dense texture to store the lighting data it generates. Bevy can be configured so that lighting is directly written to the scene's texture, which would avoid that entirely. But the way lighting is stored will be different for each 3D implementation, so this hack would only work in the context of this 2D implementation and won't generalize to 3D.
- Mouse drawing and the albedo/emissive textures are inherently dense. While the mouse is being held down, a trail is drawn and permanently stored to the appropriate dense textures. We could make it all sparse by maintaining a buffer of sprites to rebuild the scene each frame. For a proof of concept, the current approach seemed fine, but this will be explored in a 3D version.

---
# Limitations/Bugs

- A major issue with the slab-chain structure used in the Sparse model occurs when a slab is only partially populated and excess threads are left without work to do. In the `confetti` sample scene provided, thread utilization across the whole program is around 60%, which is a significant bottleneck and fixing this could result in major performance gains.
- Due to non-hardware-accelerated Rgba8Unorm compression in the Sparse model, lighting is of lower quality than in the Dense model, which uses fragment shader to store lighting in textures.
- This codebase uses a custom GPU abstraction API that wraps Bevy's own WGPU abstraction API. Bevy is not yet in 1.0, so there may be bugs. And this custom API wrapper is very much a work in progress, so this too could introduce bugs. Source code is provided, and please let me know if you do have issues or want to contribute improvements/fixes.
- Because of the way data is stored, it's not straightforward to interpolate between probes, which is a common way to smooth out lighting in conventional RC. Unclear if this is a hard limitation or if there are workarounds (this is currently unexplored).

---
# (WIP) 3D Sparse RC

The goal of this work was to test how much memory we can save with various data structures, of which the slab-chain was proven the most effective. When paired with z-order and direction-major iteration, we can avoid using a hashtable entirely, and can perfectly deduplicate tasks in just 3 lookups and comparisons. While this has proven memory-efficient in 2D, the next step is to verify it in a real 3D implementation.

---
# (WIP) Sparse "Transmissive" RC

Sparse Radiance Cascades is used as a probe-to-surface mapping function which expands rays outward to "gather" lighting, them merges that lighting and applies it to the relevant pixels. Sparse RC first casts rays from the bottom-up to generate the smallest possible merge path, then merges from the top-down along that path. This requires the storage of that merge path so it can be consumed later.

Sparse Transmissive RC casts rays out from probes that are emissive, and culls those rays for probes that are not emissive past a certain threshold. This means we can skip a lot of work for probes which contribute very little to the overall lighting of the scene. But it also eliminates the need for storing the merge path as we are no longer merging: all the lighting is applied when a ray contacts a surface.

Initially we'll only put c0 probes on voxels with emissive materials, but indirectly lit voxels will become emissive as they are illuminated over a few frames and accumulate enough light to pass the emissivity threshold. Without this threshold, static scenes would eventually emit rays from every surface that is even slightly lit.

So Transmissive RC only has a bottom-up ray casting process and eliminates the need for a top-down merge. But it necessitates some kind of "apply lighting" functionality which will need to be handled case-by-case, and may not be viable depending on the rendering pipeline setup.

Another drawback is the lack of ability to integrate skybox lighting ([ref skybox integral by Mathis](https://www.shadertoy.com/view/mtlBzX)) because we are now only casting rays for emissive parts of the scene and not all parts of the scene. This can be solved by generating an omni-directional shadow map of the scene, illuminating voxels with sky-lighting, and storing some kind of directional mask which we can check during ray-casting to cull rays that travel to the skybox. But this is highly experimental.

---
# License

All code in this repository is dual-licensed under either:
- MIT License (LICENSE-MIT or http://opensource.org/licenses/MIT)
- Apache License, Version 2.0 (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)