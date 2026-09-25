# Viboceros

An open-source CAD application in Rust, working toward a clean reimplementation
of Rhinoceros 3D. It has a modular geometry kernel, a command-driven egui/wgpu
interface, multiple viewports, snapping, layers, groups, and undo/redo.
Attribute and geometry user text can be edited with commands and retained in 3DM
files; attribute text can also be searched. `NamedView Save`, `NamedView Restore`,
and `NamedView List` manage camera views, which `Export3dm` and `Import3dm`
preserve with their construction planes. `Open3dm path` (or `Open path`) replaces
the current document; `Import3dm path` merges a 3DM into it.
`SaveAs path.3dm` names the current file; `Save` writes later changes to it and
keeps the previous version as a `.3dmbak` file. Save and export preserve the
working viewport cameras, titles, construction planes, display modes, and grid
settings. Open restores the views, layout, active viewport, maximized state,
and active layer. `ReadViewportsFromFile path.3dm` copies views and layout into
the current document. `3View`, `4View`, and `MaxViewport` change the layout;
`4View Projection=FirstAngle|ThirdAngle` restores either standard arrangement.
Plain `4View` restores the most recently selected projection and resets its views.
`SplitViewportHorizontal` and `SplitViewportVertical` divide the active view.
`NewViewport` opens a centered Top view over the model viewport area.
`CloseViewport` removes the active view, preserving covered layouts or filling tiled gaps.
The model-view tabs below the workspace select views, including views covered by
overlapping windows. Double-click a tab to rename it; right-click for view
actions, or use the mouse wheel over the tabs to cycle views.
`ViewportTabs Show|Hide|Toggle` controls the tab strip.
`ViewportTabs Align=Bottom|Top|Left|Right` moves it to a window edge.
`-ViewportProperties Title="name"` names the active view for selection and saving.
`Export3dm` leaves the current file name unchanged. `SetActiveViewport name`
selects a viewport; `SetMaximizedViewport name` selects and maximizes it. Both
also accept a viewport number.

This is an early implementation. It supports analytic and NURBS geometry,
trimmed B-reps, polygon meshes, and an expanding command set. 3DM and STL
interchange are available; STEP imports meshes or supported native planar and
NURBS B-reps and exports faceted shells or supported native B-reps. Full Rhino
compatibility is still in progress.

## Build and run

Install Rust 1.95 or newer, CMake, and a C++17 compiler, then run:

```sh
git submodule update --init --recursive
cargo run --release
```

Linux supports Wayland and X11; wgpu uses Vulkan when available. Enter commands
such as `Line 0,0,0 10,5,0`, or enter `Line` to pick points in a viewport.
`Circle 3Point 4,0,0 0,4,0 -4,0,0` constructs a circle through three world points.
`Circle 3Point 4,0,0 0,4,0 Radius=5 0,0,0` fixes its radius and center direction.
`Circle 0,0,0 Diameter=6` creates a radius-three circle; Circle also accepts
`Circumference=` and `Area=` sizes, including at its interactive prompt.
`Circle Vertical 1,2,3 5,2,3` draws a circle perpendicular to the construction
plane; enter a radius before the direction point to fix its size.
`Circle Orientation 1,2,3 1,3,3 4` chooses a plane normal from the second point.
Enter `Help` to list commands, or `Help UI` for display and drafting controls.

## Development

Release-mode tests keep the exhaustive exact-arithmetic checks practical.

```sh
cargo test --workspace --release
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
python3 -m unittest discover -s tools/rhino_oracle -t .
```

- [Command reference and examples](docs/commands/README.md)
- [Command completion, history, and file paths](docs/command-line.md)
- [Viewport controls and drafting](docs/interface.md)
- [Viewport caching and performance checks](docs/viewport-caching.md)
- [Opt-in offscreen GPU tests](docs/gpu-tests.md)
- [Imported surface shading and mesh checks](docs/imported-shading.md)
- [File formats and limitations](docs/file-formats.md)
- [Architecture and implementation status](docs/architecture.md)
- [Rhino oracle setup, Python API, and comparisons](docs/oracle.md)
