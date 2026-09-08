# Offscreen viewport tests

[Project overview](../README.md) · [Viewport controls](interface.md)

The opt-in tests render through the production wgpu pipelines into a 256×256
texture, copy the pixels to a mapped buffer, and check coverage. No application
window, desktop interaction, or Rhino process is required.

```sh
cargo test -p viboceros gpu_camera_crossing -- --ignored --nocapture
```

An available graphics adapter is required. These tests are explicitly ignored
by the ordinary suite; an explicit run fails rather than silently passing if
adapter/device creation or readback fails. The adapter and driver are printed.
wgpu instance environment settings (such as `WGPU_BACKEND`) are honored.
Readback waits have a 30-second timeout.

The face test compares a grid of pixel centers with independent ray/triangle
intersections, excluding a small barycentric edge band where rasterizer edge
rules and subpixel precision matter. It covers zero through three hidden
corners, both windings, shaded and ghosted display, and sRGB/non-sRGB targets
(32 rendered cases). Each nonempty case must contain tested covered pixels.
The wire test checks visible samples in both endpoint orders, adjacent clear
pixels, and entirely hidden segments, with both target formats (6 cases).
Both tests use the application's camera, scene submission, shaders, pipelines,
depth attachment, and buffer-upload code.

Verified on NVIDIA GB10, Vulkan, driver 610.43.02. This does not establish
other-backend parity, exact colors/lighting, overlapping transparency order,
large-coordinate accuracy, or agreement with Rhino. The tests deliberately
avoid platform-specific golden screenshots; they check geometric coverage.
