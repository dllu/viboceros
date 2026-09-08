# Offscreen viewport tests

[Project overview](../README.md) · [Viewport controls](interface.md)

The opt-in tests render through the production wgpu pipelines into a 256×256
texture, copy the pixels to a mapped buffer, and check coverage. No application
window, desktop interaction, or Rhino process is required.

```sh
cargo test -p viboceros viewport::raster_tests -- --ignored --nocapture
```

An available graphics adapter is required. These tests are explicitly ignored
by the ordinary suite; an explicit run fails rather than silently passing if
adapter/device creation or readback fails. The adapter and driver are printed.
wgpu instance environment settings (such as `WGPU_BACKEND`) are honored.
Readback waits have a 30-second timeout.
The test helper serializes graphics-context lifetimes even when the Rust test
runner uses multiple threads. A concurrent three-test run terminated with a
native SIGSEGV during this audit; the exact backend/driver cause is not isolated.
Serialization avoids concurrent adapter enumeration and device teardown in
this test harness, without changing the application renderer.
Five consecutive combined runs passed after serialization (118 renders per run).

The face test compares a grid of pixel centers with independent ray/triangle
intersections, excluding a small barycentric edge band where rasterizer edge
rules and subpixel precision matter. It covers zero through three hidden
corners, both windings, shaded and ghosted display, and sRGB/non-sRGB targets
(32 rendered cases). Each nonempty case must contain tested covered pixels.
The wire test checks visible samples in both endpoint orders, adjacent clear
pixels, and entirely hidden segments, with both target formats (6 cases).
The depth/compositing test checks center pixels in all four views with both
target formats (80 renders). A nearer opaque face must hide a farther face
regardless of insertion order, hide rear wires, and admit front wires. Ghosted
faces must combine two distinct depth-separated layers with the expected alpha
and near-layer color dominance, in either insertion order, while keeping rear
wires visible. These are nonintersecting, constant-depth faces; sorting general
intersecting transparent geometry is not established by these fixtures.
All three tests use the application's camera, scene submission, shaders, pipelines,
depth attachment, and buffer-upload code.
An ordinary non-GPU test checks the independent ray reference against analytic
hits, reversed winding, behind-origin intersections, outside barycentric weights,
parallel rays, and an unnormalized ray direction.

Verified on NVIDIA GB10, Vulkan, driver 610.43.02. This does not establish
other-backend parity, exact colors/lighting, intersecting transparency order,
large-coordinate accuracy, or agreement with Rhino. The tests deliberately
avoid platform-specific golden screenshots; they check geometric coverage.
