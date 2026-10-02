# Viboceros

An open-source CAD application in Rust, working toward a reimplementation of
Rhinoceros 3D. It combines a modular geometry kernel with a command-driven
egui/wgpu interface, multiple viewports, snapping, layers, groups, and undo/redo.

The project is in early development. Analytic and NURBS geometry, trimmed B-reps,
meshes, and 3DM/STL/STEP interchange are supported with limitations; full Rhino
compatibility is still in progress.

## Build and run

Install Rust 1.95 or newer, CMake, and a C++17 compiler, then run:

```sh
git submodule update --init --recursive
cargo run --release
```

Linux supports Wayland and X11; wgpu uses Vulkan when available.

## Documentation

- [Command reference and examples](docs/commands/README.md)
- [Viewport controls and drafting](docs/interface.md)
- [Command input, completion, history, and file paths](docs/command-line.md)
- [File formats and limitations](docs/file-formats.md)
- [Architecture and implementation status](docs/architecture.md)
- [Development and testing](docs/development.md)
- [Rhino oracle setup, Python API, and comparisons](docs/oracle.md)
