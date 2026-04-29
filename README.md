# Window and textures (Bouncy Ferris)

This example draws Ferris the crab bouncing around the screen.

In relation to previous examples it introduces rendering to window surfaces with a swapchain as well as loading images to be used as combined samplers (equivalent to textures). 

You can run this example with:

`cargo run`

By default, it runs with all validation enabled. To run without validation, take look at `run_no_validation.sh` or just execute the script.

`sh run_no_validation.sh`

## Project structure

This example is structured in different components that each do a specific function:

- `Renderer` (`src/render/renderer.rs`): Stores most Vulkan objects and functions used for rendering, however it does not perform any explicit synchronization.
- `SyncRenderer` (`src/render/sync_renderer.rs`): Manages each frame rendering operations by calling functions from `Renderer` and makes sure they are synchronized.
- `RenderInit` (`src/render/initialization/pre_window_init.rs`): Initializes some Vulkan objects that don't depend on a created window beforehand, like the instance. These objects get moved to `Renderer` afterwards.
- `<main function>`: Contains the event loop that calls for rendering repeatedly and processes window and device events.
- `Ferris` (`src/ferris.rs`): Holds Ferris's position and updates it before rendering starts.

### Windows and Vulkan

Vulkan doesn't have any mechanism for creating and managing windows, so these have to be created separately and then linked to Vulkan trough extensions. This process is quite platform dependent and so this example uses some libraries to abstract this process in a platform independent way.

 - [Winit](https://docs.rs/winit/latest/winit/index.html) provides a way to create and manage windows in a way that is mostly platform independent.
 - [raw_window_handle](https://docs.rs/raw-window-handle/latest/raw_window_handle/) provides an interface that can be used to access the handle of some window. It is implemented by winit if the `rwh_xx` cargo feature is enabled.
 - [https://docs.rs/ash-window/latest/ash_window/] incorporates `raw_window_handle` with `ash`.

You can use any windowing library other than Winit as long as you can retrieve the window handle.

### Winit

Winit uses a concept of a event loop:

```rust
 event_loop
  .run(move |event, target| match event {
    Event::AboutToWait => {
      // the application has finished processing all other events
    }
    Event::WindowEvent { event, .. } => match event {
      WindowEvent::CloseRequested => {
        // the user tried closing the window
      }
      WindowEvent::RedrawRequested => {
        // the OS or some other source requested the window to be redrawn
      }
      WindowEvent::Resized(new_size) => {
        // the window has been resized
      }
      _ => {}
    },
    _ => (),
  })
  .expect("Failed to run event loop")
```

The loop receives different events that can be processed and calls rendering continuously or only when specified.

WindowEvent::RedrawRequested is useful for applications that render irregularly or to only some parts of the screen (for example GUI applications). For other software that renders continuously it is better to do all rendering in Event::AboutToWait and synchronize internally (the case for this example). This is facilitated by `event_loop.set_control_flow(ControlFlow::Poll)`, which makes sure the AboutToWait is received continuously. 

## Initialization

### Surface and Swapchain

The `ash_window` create makes it easy to create an `vk::Surface` once the window is created. This surface is queried through the application for information related to what can be presented and appear on the screen. The crate also enumerates all instance extensions necessary for presenting and that are then used in instance creation.

When selecting the physical device, the device is checked for any formats or queues that can used in presentation to that specific surface, and most presentation operations are handled independently from others like graphics.

The swapchain is the main object that hold the images to be queued and presented in order. Using it requires enabling the `VK_KHR_swapchain` device extension. Once it is created, its holds some internally created images (can also be added externally) that can be acquired, rendered to and presented to appear on screen. Once the image is acquired it works as just any other image, so all the operations in between are no different than in offline rendering used for example in rendering the [triangle image](https://github.com/ZakStar17/ash-by-example/tree/main/triangle_image). The only thing needed is for the final layout of the image to be `PRESENT_SRC_KHR` to be usable in presentation.

The swapchain has the concept of [present modes](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkPresentModeKHR.html), that can, for example, make the GPU wait for vertical synchronization or present the image immediately. Even so, rendering and presenting don't have an intrinsic order and can happen at the same time, so synchronization objects (like semaphores) need to be used to not allow any data races.

### Texture (image sampler)

This example also creates what is usually called a texture. In Vulkan, it is just a normal image that is bound to a command buffer as a `COMBINED_IMAGE_SAMPLER` attachment. 

This mostly just requires some work to load the image file, copy the data to a host visible buffer, and then copy it to device local image with the help of some command buffers. The image view is later written to a descriptor set and used as an attachment, in this case in the fragment shader stage.

In the fragment shader: 
```glsl
layout(binding = 0) uniform sampler2D tex_sampler;

void main() {
  out_color = texture(tex_sampler, tex_coords);
}
```

For it to be used as a combined sampler, a separate `vk::Sampler` object is created. This basically works as a "function" that takes texture coordinates as input and returns the fragment color. It has multiple configuration parameters that are can instruct it to take the nearest pixel to the given coordinates or average them out.

The texture position is received as vertex data.

## Each render iteration

This example uses double buffering, meaning that in each frame the CPU is handles one set of objects and the GPU handles another set so that they can work simultaneously without much latency. In this example most of the objects are constant, other than fences and semaphores, only the graphics command pools are duplicated.

The current frame corresponds to the set of objects CPU is currently handling. The render loop involves:

 - Waiting for the current frame fence to be signaled so that the objects are safe to work with.
 - Acquiring an image to render to. It is given by the swapchain and doesn't depend on the current frame index.
 - Recording the current frame main command buffer to render to the acquired image.
 - Submitting work and presenting to the image.

In therms of synchronization, one semaphore and one fence is signaled when the graphics submission finishes. The semaphore is passed to be waited by the swapchain, and the fence is used to know when this frame objects are ready to be used by the CPU again.

Note: Recording command buffers may be somewhat resource intensive, but operations can be better optimized when recorded and submitted once if they are likely to change each recording. In this example, it is best to pass data as push constants as theses are a lot less expensive than using uniform buffers, for example.

### Push constants

In order to pass Ferris's position to the vertex shader, push constants are used. These are small amounts of data that are passed during command buffer recording and read by the shader each time it executes. In order to use them, their amount just has to be indicated during pipeline layout creation and then can be safely added with `device.cmd_push_constants()`.

In the shader, they can be used simply with:

```glsl
layout(push_constant) uniform PushConstantData {
  vec2 position;
  vec2 ratio; // relative width and height
} pc;
```

## Handling window resizes and out of date surfaces

A surface can become suboptimal or incompatible with the current swapchain. This results in having to query the new surface capabilities and recreate the swapchain and all objects like framebuffers that depend on the its images.

In this example the pipeline doesn't use dynamic state for the viewport, meaning that it also has to be recreated if the window size has changed.

Thankfully, these most of these objects have the concept of being "old" or "expired", meaning they can still be used in rendering, but no new submissions that use them can be submitted and they are expected be destroyed once they become inactive. This means that most of the time it is still possible to render continuously by keeping track of old objects, only having rare cases where for example the swapchain image format changes that will involve waiting for all submissions to complete to be able to recreate safely the render pass.

## Cargo features

This example implements the following cargo features:

- `vl`: Enable validation layers.
- `load`: Load the system Vulkan Library at runtime.
- `link`: Link the system Vulkan Library at compile time.
- `log_alloc`: Log allocations in a friendly manner.

`vl`, `load` and `log_alloc` are enabled by default. To disable them, pass `--no-default-features` to cargo.

For more information about linking and loading check
[https://docs.rs/ash/latest/ash/struct.Entry.html](https://docs.rs/ash/latest/ash/struct.Entry.html).