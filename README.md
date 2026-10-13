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
git submodule update --init third_party/opennurbs
cargo run --release
```

Linux supports Wayland and X11; wgpu uses Vulkan when available.

For native B-rep modeling, NURBS editing and fitting, Sweep1/Pipe construction,
and STEP material regions on Linux,
install oneTBB development libraries (`libtbb-dev` on Ubuntu) and enable usd-brep:

```sh
GIT_LFS_SKIP_SMUDGE=1 git submodule update --init third_party/usd-brep
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo run --release --features native-smlib
```

## Documentation

- [Command reference and examples](docs/commands/README.md)
- [Viewport controls and drafting](docs/interface.md)
- [Command input, completion, history, and file paths](docs/command-line.md)
- [File formats and limitations](docs/file-formats.md)
- [Analytic normals and tessellation provenance](docs/tessellation-normals.md)
- [Architecture and implementation status](docs/architecture.md)
- [Modelling issue fixes and regression results](docs/modelling-issues.md)
- [Development and testing](docs/development.md)
- [USD BRep kernel integration](docs/usd-brep.md)
- [USD BRep limitations and bugs](docs/usd-brep-limitations.md)
- [Native lamp-well conversion regression](docs/native-lamp-well.md)
- [Rhino oracle setup, Python API, and comparisons](docs/oracle.md)
- [Large-model display mesh diagnostics](docs/display-mesh-diagnostics.md)
