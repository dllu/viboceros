# Viboceros
<img width="1178" height="814" alt="Viboceros viewports and command interface" src="https://github.com/user-attachments/assets/cb4f97f2-3283-45d3-91e6-93583e779ea7" />

An open-source CAD application in Rust, working toward a reimplementation of
Rhinoceros 3D. It combines usd-brep/SMLib modeling with a command-driven
egui/wgpu interface, multiple viewports, snapping, layers, groups, and undo/redo.

The project is in early development. Analytic and NURBS geometry, trimmed B-reps,
meshes, SubDs, and 3DM/STL/STEP interchange are supported with limitations; full Rhino
compatibility is still in progress.

## Build and run

The current build supports Linux with Wayland or X11. Install Rust 1.95 or
newer, CMake, a C++17 compiler, `patch`, and oneTBB development libraries
(`libtbb-dev` on Ubuntu).

```sh
GIT_LFS_SKIP_SMUDGE=1 git submodule update --init third_party/opennurbs third_party/usd-brep
cargo run --release
```

All builds link usd-brep. Set `CMAKE_BUILD_PARALLEL_LEVEL` to limit native build
jobs if needed. wgpu uses Vulkan when available.

## Documentation

[Headless Python modeling](docs/python.md) covers scripting commands, selecting
geometry, exchanging files, and capturing shaded views.

[Documentation](docs/README.md) covers usage, commands, architecture, and testing.
