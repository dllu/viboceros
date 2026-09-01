# Viboceros

Viboceros is an early-stage, cross-platform CAD application written in Rust. It
is organized around independent geometry, document, and command crates, with an
egui interface rendered by wgpu.

The current foundation supports finite 3D points, vectors, line segments,
planes, bounding boxes, rational NURBS curves with analytic first derivatives,
validated triangle meshes, layers, groups, and bounded undo/redo. The top
viewport can pan and zoom in wireframe, shaded, or ghosted mode. Its command
line currently accepts:

```text
Point 1,2,0
Line 0,0,0 10,5,0
ControlPointCurve 3 0,0 2,3 5,3 8,0
Layer Construction
Clear
Undo
Redo
ImportStl path/to/model.stl
ExportStl Binary path/to/model.stl
ExportStl Ascii path/to/model.stl
Help
```

## Build and run

Install a current stable Rust toolchain (Rust 1.95 or newer), then run:

```sh
cargo run
```

On Linux, wgpu uses Vulkan when available and the window supports both Wayland
and X11. Run all workspace tests and checks with:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Both ASCII and binary STL are supported. 3DM, STEP, and production surface and
solid modelling are not implemented yet.
