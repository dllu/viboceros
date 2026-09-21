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

Documents, history, and display caches share immutable `GeometrySnapshot` handles.
The UI checks snapshot storage identity in constant time per object, without
cloning or comparing mesh vertices or curve controls on stationary repaints.
Every geometry edit installs a new snapshot; undo/redo and rollback restore the
corresponding retained snapshot. Document clones share unchanged geometry, while
independent edits replace only their own handles. Retaining ownership makes this
safe against address reuse; an object ID or an unowned pointer would not suffice.
Snapshot value equality remains structural, independently of storage identity.
Even an explicit equal-geometry replacement may invalidate display data safely.

Wire density and model tolerance also participate in invalidation. Display colors,
visibility, locking, selection, and object order are checked separately from geometry, so
style-only changes do not repeat tessellation. Deleted or hidden objects are
removed from the shared cache on the next scene preparation. Each viewport and
GPU slot retains at most its most recent scene.

`Object::geometry()` still borrows the geometry; `geometry_snapshot()` exposes
the shared immutable handle. There is no mutable snapshot access or geometry
revision counter. History and cloned documents also avoid copying unchanged
geometry payloads, at the cost of one shared allocation/indirection per new value.
Metadata, object iteration, and per-object cache lookup remain work on every repaint.
Other costs include per-primitive camera/depth preparation and uploads for a moving
view, triangle sorting in ghosted mode, tessellation memory, and four prepared
scenes. Geometry stays alive while history, a reader, or a cached scene owns it.
This is not an asynchronous mesher or a camera-uniform-only renderer:
current large-coordinate precision guarantees require view-dependent f64
rebasing and depth encoding before f32 GPU submission.

## Verification

```sh
cargo test --release -p viboceros
cargo test --release -p viboceros viewport::raster_tests -- --ignored --nocapture
cargo test --release -p viboceros benchmark_cached_viewport_frames -- --ignored --nocapture
cargo test --release -p viboceros benchmark_large_geometry_snapshot_frames -- --ignored --nocapture
cargo test --release -p viboceros-document geometry_snapshot
cargo clippy --release -p viboceros --all-targets -- -D warnings
cargo fmt --all -- --check
```

Ordinary tests cover four-view sharing, stationary scene identity, navigation,
edits/undo/redo/rollback, cloned documents, layer and style changes, tolerance, deletion,
lazy wireframe behavior, and selection against the original uncached projection
in every view and display mode. Snapshot tests additionally cover equal replacements,
failed edit batches, transforms, morphs, unit changes, and independent layer copies.
Hidden-layer unit conversion and its history must not restore stale display geometry.
The additional GPU test checks real pixels after edit/undo/deletion and layer hide/show,
and asserts that stationary frames and unchanged document clones perform no buffer uploads.
The existing GPU precision, clipping, and compositing tests remain applicable.

The opt-in benchmark reports cold, cached stationary, and cached orbit CPU scene
preparation for 13 objects in four shaded views. It excludes geometry creation,
egui layout, GPU submission, and presentation; its timings are not application
FPS or a comparison with Rhino. It has no timing threshold in the ordinary suite.

A release run with shared snapshots on the development host measured 17.15 ms for
the first four-view preparation, 0.0069 ms per stationary frame, and 0.96 ms per orbit
frame (100 cached frames per measurement). These cold-versus-cached measurements
are not a timed comparison with the old renderer; first-frame meshing cost remains.

The separate large-source benchmark creates one 100,000-vertex polyline and warms
four wireframe views. It measures 100 stationary frames and 100 document clones,
excluding geometry creation and cold scene preparation. Before the snapshot change
(`20f9246` plus this benchmark), the same test measured 394.398 µs/frame and
45.593 µs/clone. Three post-change runs measured 0.295–0.628 µs/frame and
0.151–0.269 µs/clone; the final isolated benchmark measured 0.299 and 0.206 µs.
This narrow comparison demonstrates removal of payload copies/scans; it is not a
general scene/frame budget, end-to-end FPS, or a comparison with Rhino. The one-object
fixture does not measure scaling with object count or geometry-changing workloads.

Verification checkpoint: 3,088 release-mode workspace tests passed (26 opt-in
exclusions), plus all seven explicit GPU tests on NVIDIA GB10/Vulkan, driver
610.43.02. The 290 Python tests, formatting, and warnings-denied Clippy/Rustdoc
checks also passed. No new Rhino timing or viewport comparison is claimed.
