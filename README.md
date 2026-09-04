# Viboceros

An open-source CAD application in Rust, working toward a clean reimplementation
of Rhinoceros 3D. It has a modular geometry kernel, a command-driven egui/wgpu
interface, four viewports, snapping, layers, groups, and undo/redo.

This is an early implementation. It supports analytic and NURBS geometry,
trimmed B-reps, polygon meshes, and an expanding command set. 3DM and STL
interchange are available; STEP currently imports tessellated geometry and
exports faceted shells. Full Rhino compatibility is still in progress.

## Build and run

Install Rust 1.95 or newer, CMake, and a C++17 compiler, then run:

```sh
git submodule update --init --recursive
cargo run --release
```

Linux supports Wayland and X11; wgpu uses Vulkan when available. Enter commands
such as `Line 0,0,0 10,5,0`, or enter `Line` to pick points in a viewport.
Enter `Help` to list commands.

## Development

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
python3 -m unittest discover -s tools/rhino_oracle -t .
```

- [Command reference and examples](docs/commands/README.md)
- [Viewport controls and drafting](docs/interface.md)
- [File formats and limitations](docs/file-formats.md)
- [Architecture and implementation status](docs/architecture.md)
- [Rhino oracle setup, Python API, and comparisons](docs/oracle.md)
