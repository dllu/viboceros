# Viboceros
<img width="1178" height="814" alt="Viboceros viewports and command interface" src="https://github.com/user-attachments/assets/cb4f97f2-3283-45d3-91e6-93583e779ea7" />

An open-source CAD application in Rust, working toward a reimplementation of
Rhinoceros 3D. It combines a modular geometry kernel with a command-driven
egui/wgpu interface, multiple viewports, snapping, layers, groups, and undo/redo.

The project is in early development. Analytic and NURBS geometry, trimmed B-reps,
meshes, SubDs, and 3DM/STL/STEP interchange are supported with limitations; full Rhino
compatibility is still in progress.

## Build and run

Install Rust 1.95 or newer, CMake, and a C++17 compiler, then run:

```sh
git submodule update --init third_party/opennurbs
cargo run --release
```

Linux supports Wayland and X11; wgpu uses Vulkan when available.

For native B-rep modeling and expanded NURBS/STEP support on Linux,
install `patch` and oneTBB development libraries (`libtbb-dev` on Ubuntu) and enable usd-brep:

```sh
GIT_LFS_SKIP_SMUDGE=1 git submodule update --init third_party/usd-brep
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo run --release --features native-smlib
```

## Documentation

[Documentation](docs/README.md) covers usage, commands, architecture, and testing.
