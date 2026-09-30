# Viboceros

An open-source CAD application in Rust, working toward a clean reimplementation
of Rhinoceros 3D. It has a modular geometry kernel, a command-driven egui/wgpu
interface, multiple viewports, snapping, layers, groups, and undo/redo.
Attribute and geometry user text can be edited with commands and retained in 3DM
files; attribute text can also be searched. `NamedView Save`, `NamedView Restore`,
`NamedView List`, and `NamedView Import path.3dm` manage camera views.
`Export3dm` and `Import3dm` preserve them with their construction planes.
`NamedCPlane Save`, `Restore`, and `Import path.3dm` manage reusable construction
planes through 3DM files.
`Open3dm path` (or `Open path`) replaces
the current document; `Import3dm path` merges a 3DM into it.
`SaveAs path.3dm` names the current file; `Save` writes later changes to it and
keeps the previous version as a `.3dmbak` file. Save and export preserve the
working viewport cameras, titles, construction planes, display modes, and grid
settings. Open restores the views, layout, active viewport, maximized state,
and active layer. `ReadViewportsFromFile path.3dm` copies views and layout into
the current document. `3View`, `4View`, and `MaxViewport` change the layout;
`4View Projection=FirstAngle|ThirdAngle` restores either standard arrangement.
`SynchronizeCPlanes [source] [SetView=Yes|No]` aligns standard view planes to a
source viewport's construction plane.
`CopyCPlaneToAll` and `CopyCPlaneSettingsToAll` copy from a picked viewport;
press Enter to use the active viewport.
`CPlane All point` and `CPlane Through All point` move every viewport's
construction plane origin while retaining its own axes.
At either point prompt, `All` toggles its setting; `All=Yes|No` selects it.
`CPlane View` aligns the active plane to its camera without moving the view.
`CPlane Object` aligns it to a line, polyline, polycurve, NURBS curve, circle, arc,
ellipse, surface, or picked mesh/B-rep face.
`CPlane Curve` places it perpendicular to a curve at a picked station.
`CPlane Surface` places a tangent plane at a chosen point on a surface or B-rep face, with `IgnoreTrims` for underlying-surface picks.
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
Select two open curves, then use `Match Continuity=Tangency PreserveOtherEnd=Position`
to edit the first curve at the nearest pair of ends. `Pick1=` and `Pick2=` choose
other ends; single-span curves currently support position, tangent, and curvature
matching.
Multi-span curves also support position matching with endpoint trimming, and
tangent or curvature matching. When the source has too few controls to preserve
the requested opposite end, Match adds controls before editing its end. For
five-control cubic and four- or five-control quadratic G2 matches preserving
far G2, it uses a uniform curve fitted at Greville parameters; other short
sources use knot insertion.
`AverageCurves=Yes` moves both curves for position, tangent, or curvature
matching and keeps the change in one undo step. Average curvature matching can
preserve the far position or tangent of multi-span curves.
It also preserves far curvature on both edited multi-span curves, refining
either curve when needed.
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
