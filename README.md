# Triangle image

This example draws a triangle on an image, copies it to host accessible memory and saves it to a file.

It uses [Image clear example](https://github.com/zzstar17/ash-by-example/tree/main/compute_image_clear) as base and other concepts from [Storage image compute shader](https://github.com/zzstar17/ash-by-example/tree/main/storage_image_compute_shader). The compute pipeline is substituted with a graphics pipeline and it introduces render passes, vertex and index buffers.

You can run this example with:

`cargo run`

By default, it runs with all validation enabled. To run without validation, take look at `run_no_validation.sh` or just execute the script.

`sh run_no_validation.sh`

## Shaders

This example uses one vertex and one fragment shader. These just take one 2D position and one color per vertex and assign them as is in 3D coordinates.

The vertex type is:

```rust
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct Vertex {
  pub pos: [f32; 2],
  pub color: [f32; 3],
}
```

One in the final buffer, the vertices are read from continuous memory by the GPU.

## Code overview

- A render pass describes how image attachments are used through rendering. This is similar to creating pipeline barriers to transition image layouts and creating memory dependencies between stages, however it all needs to be specified before pipeline creation. It has multiple execution steps called subpasses. In this example, the render pass contains only one subpass and one attachment (the local image), as well as two memory dependencies for external synchronization to and from the subpass.
- A framebuffer which is compatible with the render pass is created. This framebuffer takes a image view from the local image as an attachment to be used in rendering.
- The two shaders are loaded and passed to the graphics pipeline creation, which creates configurations about used vertex and index parameters, as well as other configurations for fixed functions in the pipeline. These are mostly kept to a minimum to allow drawing triangles on a 2D plane.
- The graphics command pool is created. Because this example doesn't use dynamic state for the pipeline, mostly everything is already configured, so the buffer just needs to bind the pipeline, vertex and index buffers and issue the draw command. After the render pass ends the image is already in its final layout for transfer, so it just needs to be released and can be used in the transfer command buffer as usual.
- The buffers are created and allocated in one device local memory. In order to populate them with data, an identical pair of buffers is created in host visible memory. These are mapped, the data is copied, and a set of [vkCmdCopyBuffer](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/vkCmdCopyBuffer2.html) operations is submitted to finally the data from the host visible to the local final buffers. This involves more work but makes the final buffers available in a more accessible local memory for the GPU.
- The work is then submitted and saved in the same fashion as in [Image clear](https://github.com/zzstar17/ash-by-example/tree/main/compute_image_clear).

## Cargo features

This example implements the following cargo features:

- `vl`: Enable validation layers.
- `load`: Load the system Vulkan Library at runtime.
- `link`: Link the system Vulkan Library at compile time.
- `log_alloc`: Log allocations in a friendly manner.

`vl`, `load` and `log_alloc` are enabled by default. To disable them, pass `--no-default-features` to cargo.

For more information about linking and loading check
[https://docs.rs/ash/latest/ash/struct.Entry.html](https://docs.rs/ash/latest/ash/struct.Entry.html).
