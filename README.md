# Viboceros
<img width="1178" height="814" alt="C2C04CCF-0E74-4702-AB38-8A875DF4BCC7" src="https://github.com/user-attachments/assets/cb4f97f2-3283-45d3-91e6-93583e779ea7" />

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

For native B-rep modeling, curved Project targets, NURBS editing, extension and
fitting, Sweep1/Pipe construction, and STEP material regions on Linux,
install `patch` and oneTBB development libraries (`libtbb-dev` on Ubuntu) and enable usd-brep:

```sh
GIT_LFS_SKIP_SMUDGE=1 git submodule update --init third_party/usd-brep
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo run --release --features native-smlib
```

## Documentation

- [Command reference and examples](docs/commands/README.md)
- [Directional projection, custom points and subcurves](docs/commands/project.md)
- [Infinite-plane projection and input](docs/commands/project-infinite-plane.md)
- [IPlane intersections and shared plane picking](docs/commands/iplane.md)
- [Pull curves and points toward surfaces and meshes](docs/commands/pull.md)
- [Loose projection and pulling through edit points](docs/commands/projection-loose.md)
- [Control and edit-point workflows](docs/control-points.md)
- [SetPt coordinate alignment for objects and grips](docs/commands/set-point.md)
- [Mesh curve pulling and qualifications](docs/commands/pull-mesh.md)
- [Mesh projection and qualification](docs/commands/project-mesh.md)
- [Bounded curve approximation and mesh Pull preparation](docs/curve-approximation.md)
- [Viewport controls and drafting](docs/interface.md)
- [Command input, completion, history, and file paths](docs/command-line.md)
- [File formats and limitations](docs/file-formats.md)
- [Analytic normals and tessellation provenance](docs/tessellation-normals.md)
- [Architecture and implementation status](docs/architecture.md)
- [Modelling issue fixes and regression results](docs/modelling-issues.md)
- [Development and testing](docs/development.md)
- [USD BRep kernel integration](docs/usd-brep.md)
- [Native parallel-view silhouettes](docs/commands/silhouette.md)
- [Native surface-normal pulling foundation](docs/smlib-surface-pull.md)
- [USD BRep limitations and bugs](docs/usd-brep-limitations.md)
- [Native NURBS offsets and qualification](docs/smlib-curve-offset.md)
- [Native curve intersections and qualification](docs/smlib-curve-intersections.md)
- [Native curve self-intersections and qualification](docs/smlib-curve-self-intersections.md)
- [Native curve/surface intersections and qualification](docs/smlib-curve-surface-intersections.md)
- [Native general surface intersections, face trims and tangencies](docs/smlib-surface-surface-intersections.md)
- [Native lamp-well conversion regression](docs/native-lamp-well.md)
- [Rhino oracle setup, Python API, and comparisons](docs/oracle.md)
- [Large-model display mesh diagnostics](docs/display-mesh-diagnostics.md)
