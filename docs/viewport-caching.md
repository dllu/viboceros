# Viewport display caching

[Project overview](../README.md) · [Viewport controls](interface.md)

The four viewports share camera-independent display geometry: sampled curves,
surface/B-rep wire curves, mesh edges, tessellated faces, and smooth corner
normals. Wireframe views generate only wires; shaded faces and normals are lazy.
Click and window selection reuse this data. Point-cloud click picking retains
its existing spatial index.

Each viewport also retains its last prepared GPU scene. Unchanged geometry,
styles, camera, viewport rectangle, and display mode reuse that scene. The GPU
renderer retains the scene identity alongside its buffers and skips all buffer
uploads when it receives the same scene again. Orbiting one view rebuilds only
that view's scene, while sharing the existing tessellation and curve samples.
Drafting/grid/selection overlays still update independently through egui.

The document currently exposes no geometry revision counter. The UI therefore
keeps an owned geometry snapshot per cached object and compares it to the live
source before reuse. Object ID or memory address alone would miss replacement,
undo/redo, rollback, and independent edits to cloned documents. Wire density and
model tolerance also participate in invalidation. Display colors, visibility,
locking, selection, and object order are checked separately from geometry, so
style-only changes do not repeat tessellation. Deleted or hidden objects are
removed from the shared cache on the next scene preparation. Each viewport and
GPU slot retains at most its most recent scene.

This deliberately changes no document or geometry-kernel API. Remaining costs
include value comparisons on repaint, per-primitive camera/depth preparation and
uploads for a moving view, and triangle sorting in ghosted mode. Large models
also need memory for source snapshots, tessellation, and the four prepared
scenes. This is not an asynchronous mesher or a camera-uniform-only renderer:
current large-coordinate precision guarantees require view-dependent f64
rebasing and depth encoding before f32 GPU submission.

## Verification

```sh
cargo test --release -p viboceros
cargo test --release -p viboceros viewport::raster_tests -- --ignored --nocapture
cargo test --release -p viboceros benchmark_cached_viewport_frames -- --ignored --nocapture
cargo clippy --release -p viboceros --all-targets -- -D warnings
cargo fmt --all -- --check
```

Ordinary tests cover four-view sharing, stationary scene identity, navigation,
edits/undo/redo/rollback, cloned documents, style changes, tolerance, deletion,
lazy wireframe behavior, and selection against the original uncached projection
in every view and display mode. The additional GPU test checks real pixels after
edit/undo/deletion and asserts that stationary frames perform no buffer uploads.
The existing GPU precision, clipping, and compositing tests remain applicable.

The opt-in benchmark reports cold, cached stationary, and cached orbit CPU scene
preparation for 13 objects in four shaded views. It excludes geometry creation,
egui layout, GPU submission, and presentation; its timings are not application
FPS or a comparison with Rhino. It has no timing threshold in the ordinary suite.

A release run on the development host measured 16.83 ms for the first four-view
preparation, 0.012 ms per stationary frame, and 1.16 ms per orbit frame (100
cached frames per measurement). All 368 ordinary application tests and seven
explicit GPU tests passed; GPU tests used NVIDIA GB10/Vulkan, driver 610.43.02.
These cold-versus-cached measurements are not a timed comparison with the old
renderer, and first-frame meshing cost remains.
