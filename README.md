# Viboceros

Viboceros is an early-stage, cross-platform CAD application written in Rust. It
is organized around independent geometry, document, drafting, command, and
file-format crates, with an egui interface rendered by wgpu.

The current foundation supports finite 3D points, vectors, line segments,
analytic circles, circular arcs, and ellipses, validated open and closed
polylines, planes, bounding boxes, rational NURBS curves with analytic first
derivatives, rational NURBS surfaces with analytic partial derivatives,
validated triangle meshes, layers, groups, and bounded undo/redo.
The top viewport can pan and zoom in wireframe, shaded, or ghosted mode. Its
command line currently accepts:

```text
Point 1,2,0
Line 0,0,0 10,5,0
Circle 0,0,0 5
Arc 5,0,0 0,5,0 -5,0,0
Ellipse 0,0 6,0 0,3
Polyline 0,0 4,0 4,3 7,3
Rectangle 0,0 8,5
Polygon 6 0,0 5
ControlPointCurve 3 0,0 2,3 5,3 8,0
SrfPt 0,0,0 8,0,0 8,5,2 0,5,2
Layer New Construction
Layer Hide Construction
Layer Show Construction
Layer Current Default
SelAll
Invert
Move 0,0,0 5,0,0
Copy 5,0,0 5,5,0
Scale 0,0 2
Rotate 0,0 45
Mirror 0,-5 0,5
Group Assembly
Group All Everything
Ungroup
Ungroup Assembly
Join
Explode
Length
Area
Divide 8
Divide Length 2.5 MarkEnds
Delete
Clear
Undo
Redo
ImportStl path/to/model.stl
ExportStl Binary path/to/model.stl
ExportStl Ascii path/to/model.stl
ImportStep path/to/model.step
ExportStep path/to/model.step
Import3dm path/to/model.3dm
Export3dm path/to/model.3dm
Help
```

Enter `Point`, `Line`, `Circle`, `Arc`, `Ellipse`, `Polyline`, `Rectangle`,
`Polygon`, or `SrfPt` without coordinates to pick points in the viewport;
press Enter to finish a polyline. `Polygon` defaults to four sides, or accepts
a side count such as `Polygon 6`. With objects selected, enter `Move` or `Copy`
to pick a base and destination point, `Scale` or `Rotate` to pick
center/reference/target points, or `Mirror` to pick a two-point axis. `Join`
connects unambiguous line/polyline endpoint chains within the document
tolerance, while `Explode` turns polylines back into attribute-preserving line
segments. `Length` measures analytic, polyline, and NURBS curves with controlled
accuracy; `Area` measures circles, ellipses, closed planar polylines, and meshes.
`Divide` creates equal arc-length points on selected curves by segment count or
requested segment length; add `MarkEnds` to include open-curve endpoints.
Osnap captures visible Point, End, Mid, Center, and Quad features;
SmartTrack captures horizontal and vertical alignment from the first picked
point. Drag with the middle mouse button to pan while a drafting command is
active. Outside a drafting command, click geometry to select its connected
group, Shift-click to add, and Ctrl-click or Command-click to toggle. Click
empty space or press Esc to clear the selection; press Delete to remove
selected objects.

## Build and run

Install a current stable Rust toolchain (Rust 1.95 or newer), CMake, and a C++17
compiler. Initialize the pinned OpenNURBS submodule after cloning, then run:

```sh
git submodule update --init --recursive
cargo run
```

On Linux, wgpu uses Vulkan when available and the window supports both Wayland
and X11. Run all workspace tests and checks with:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Rhino geometry oracle

The versioned Python oracle API runs identical JSON geometry batches in a
native release build of Viboceros and Rhino 8, recursively checks every numeric
result, and reports per-operation timings. With Rhino installed through the
configured Wine/FEX launcher, run the core fixture with:

```sh
python3 -m tools.rhino_oracle compare \
  tools/rhino_oracle/fixtures/core.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12
```

The same workflow is importable for instrumentation and tests:

```python
from tools.rhino_oracle import OracleClient, load_request

report = OracleClient().compare(load_request("tools/rhino_oracle/fixtures/core.json"))
assert report.passed
```

Set `VIBOCEROS_RHINO_LAUNCHER` to use another launcher. McNeel's documented
`/runscript` startup interface is tried first; this project's Wine path also
has an owned-window fallback that requires `wmctrl` and `xdotool` and never
targets a pre-existing Rhino process. Set `VIBOCEROS_RHINO_UI_FALLBACK=0` to
disable it. The `viboceros` and `rhino` modes run either side independently.
Timings cover repeated geometry/API calls after one warm-up and exclude process
startup, object construction, and JSON I/O; Rhino timings include its public
Python/RhinoCommon bridge.

Both ASCII and binary STL are supported. Initial 3DM import/export uses McNeel's
OpenNURBS toolkit and preserves points, lines, NURBS curves, untrimmed NURBS
surfaces, triangle meshes, layer state, and object state. Circles, arcs,
ellipses, and polylines are exported without approximation as rational NURBS
curves; canonical degree-one curves return as editable polylines. Unsupported
trimmed B-rep and solid objects are counted and reported during import. Initial
STEP interchange uses the Apache-2.0 Monstertruck kernel to read solid/shell
B-reps and assemblies, apply instance transforms, and robustly tessellate exact
trimmed surfaces into validated display meshes. Parser, topology, and
unsupported-representation losses are reported instead of being silent. STL and
STEP export tessellate visible NURBS surfaces; STEP writes the results as faceted
shells with shared topology and planar faces. Editable STEP B-reps, trimmed 3DM
B-rep/solid interchange, groups in 3DM, and production surface and solid
modelling are not implemented yet.
