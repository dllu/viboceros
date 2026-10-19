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

For [native B-rep modeling](docs/smlib-import.md), curved Project targets, NURBS editing, extension and
fitting, Sweep1/Sweep2/Pipe/RailRevolve construction, and STEP material regions on Linux,
install `patch` and oneTBB development libraries (`libtbb-dev` on Ubuntu) and enable usd-brep:

```sh
GIT_LFS_SKIP_SMUDGE=1 git submodule update --init third_party/usd-brep
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo run --release --features native-smlib
```

## Documentation

- [Command reference and examples](docs/commands/README.md)
- [Orient and Orient3Pt point placement](docs/commands/orient.md), plus
  [curve, edge, and temporary-portion placement](docs/commands/orient-on-curve.md)
  and [surface placement, XYZ presets and per-placement getters](docs/commands/orient-on-surface.md)
- [Remap objects between construction planes](docs/commands/remap-cplane.md), with viewport, world and saved-plane targets and Python replay
- [Flow objects between curves](docs/commands/flow.md), with TargetSurface orientation, Local falloff, spatial Roadlike axes and grip previews
- [Conic curves](docs/commands/conic.md), with tangent/perpendicular endpoints, surface edges and direction-locked temporary portions
- Interactive [parabolas](docs/commands/parabola.md) and [hyperbolas](docs/commands/hyperbola.md), with construction modes, focus previews and numeric getters
- [Ellipse construction](docs/commands/ellipse.md), including AroundCurve, deformable cubics, numeric axis locks and focus previews
- [Tangent arcs](docs/commands/arc-tangent.md), with curve/edge contacts, radius locks and arc-choice previews
- [Tilted arcs](docs/commands/arc-tilted.md), with a spatial orientation rectangle, signed sweeps and measured lengths
- [Deformable arcs](docs/commands/arc-deformable.md), with editable NURBS output, degree/count settings and shared previews
- [Arc extensions](docs/commands/arc-extension.md), with curve/edge endpoints, normal-plane centers and source-preserving previews
- [Circle construction](docs/commands/circle-deformable.md), with variable-degree periodic/clamped output,
  [Tangent](docs/commands/circle-tangent.md), FitPoints and
  [AroundCurve](docs/commands/circle-around-curve.md) curve/edge guides
- [Interactive catenaries](docs/commands/catenary.md), with four constraint modes, sag-side picking, endpoint End snaps and apex previews
- [Interactive helices and spirals](docs/commands/helix-spiral.md), with radius locks, Turns/Pitch, Flat/Vertical previews and AroundCurve rail/edge picking
- [Rectangle construction modes](docs/commands/rectangle.md), with circular and conic corner previews
- [Flow objects between surfaces](docs/commands/flow-along-surface.md), with corner/edge orientation, rigid group origins, shared Rectangle input modes, normal constraints, and grip previews
- Fit [planes](docs/commands/plane-through-points.md) and [lines](docs/commands/line-through-points.md) through points, clouds and grips
- Fit [spheres through points and grips](docs/commands/sphere-fit-points.md)
- [Point deviation analysis and distance display](docs/commands/point-deviation.md)
- [Curve deviation reports, temporary portions, and compatibility limits](docs/commands/crv-deviation.md)
- [Simplify curve representations while retaining domains and document identity](docs/commands/simplify-crv.md)
- [Resize arcs and circles by radius, diameter, area or circumference](docs/commands/modify-radius.md)
- [Delete curve portions with point or numeric input](docs/commands/delete-subcurve.md)
- [Mark conic foci and optional axes](docs/commands/mark-foci.md)
- [Reduce polyline vertices with tolerance and Copy options](docs/commands/reduce-polyline.md)
- [Persistent curvature graphs with adaptive outlines](docs/commands/curvature-graph.md) and [outline extraction](docs/commands/extract-curvature-graph.md)
- [Native geometry checks and bad-object selection](docs/commands/check.md)
- [Degenerate mesh face cleanup](docs/commands/cull-degenerate-mesh-faces.md)
- [Divide surfaces and polysurfaces along creases](docs/commands/divide-along-creases.md),
  using [paired native isocurves](docs/native-isocurve-pairs.md)
- [Surface fillets with trim/split, joining and previews](docs/commands/fillet-srf.md),
  preserving [original parent UV charts](docs/fillet-parent-charts.md)
- [Reparameterize curves, trimmed surfaces and placed block geometry with viewport picks and shared numeric prompts](docs/commands/reparameterize.md),
  including [Automatic surface domains](docs/automatic-surface-domains.md)
- [Sweep cross sections along two rails](docs/commands/sweep2.md)
- [Revolve a profile along a rail](docs/commands/rail-revolve.md)
- [Gordon networks](docs/smlib-gordon-network.md) and [automatic curve fitting](docs/automatic-gordon-network-fit.md)
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
- [Preserving rational NURBS storage inside polycurves and B-rep edges](docs/nested-rational-3dm.md)
- [Analytic normals and tessellation provenance](docs/tessellation-normals.md)
- [Native curve/surface endpoint evaluation and one-sided derivatives](docs/native-basis-evaluation.md),
  with [interpolation at supplied parameters](docs/native-point-interpolation.md),
  [exact curve/surface splits](docs/native-exact-splits.md)
  and [tensor Bezier decomposition](docs/native-tensor-bezier.md)
- [Architecture and implementation status](docs/architecture.md)
- [Modelling issue fixes and regression results](docs/modelling-issues.md)
- [Development and testing](docs/development.md)
- [Prepared curve-length queries and integration diagnostics](docs/prepared-curve-length.md)
- [Unit directions and model tolerances](docs/unit-directions-and-model-tolerance.md)
- [Primitive directions and construction-plane parameter units](docs/direction-adapters.md)
- [Surface frames, arrays and orientation under UV rescaling](docs/surface-parameter-frames.md)
- [Pullback parameter units and UV border domains](docs/pullback-parameter-units.md)
- [Curve-array frames and Flow direction units](docs/curve-direction-units.md)
- [USD BRep kernel integration](docs/usd-brep.md)
- [Native surface-to-surface fillet producer](docs/native-surface-fillets.md), with
  [UV pick sides and construction accuracy](docs/native-surface-fillet-picks.md) and
  [parent trimming/splitting](docs/native-surface-fillet-parents.md)
- [Circular fillet rail correspondence](docs/native-circular-fillet-rails.md)
- [Native surface joining](docs/smlib-joining.md): [UV transposes](docs/native-uv-transpose.md)
  and [closed-component orientation](docs/native-closed-join.md)
- [Native pole transfer and exact NURBS copies](docs/native-pole-transfer.md)
- [Native parallel-view silhouettes](docs/commands/silhouette.md)
- [Native tapered curve extrusion](docs/commands/extrude-crv-tapered.md)
- Native surface extrusion: [straight](docs/commands/extrude-srf.md),
  [along curves](docs/commands/extrude-srf-along-crv.md), [to a point](docs/commands/extrude-srf-to-point.md)
  and [tapered](docs/commands/extrude-srf-tapered.md)
- [Native edge chamfers](docs/commands/chamfer-edge.md) and [G2 edge blends](docs/commands/blend-edge.md)
- [Coplanar face merging for meshes and B-reps](docs/commands/merge-all-coplanar-faces.md)
- [Merge a selected face pair or connected planar region](docs/commands/merge-coplanar-face.md)
- [Shell solid walls and face openings](docs/commands/shell.md)
- [Native surface-normal pulling foundation](docs/smlib-surface-pull.md)
- [USD BRep limitations and bugs](docs/usd-brep-limitations.md)
- [Native NURBS offsets and qualification](docs/smlib-curve-offset.md)
- [Native curve intersections and qualification](docs/smlib-curve-intersections.md)
- [Native in-tolerance curve contacts](docs/native-curve-contact-tolerance.md)
- [Native UV drop accuracy](docs/native-drop-curve-accuracy.md)
- [Native curve self-intersections and qualification](docs/smlib-curve-self-intersections.md)
- [Native curve/surface intersections and qualification](docs/smlib-curve-surface-intersections.md)
- [Native general surface intersections, face trims and tangencies](docs/smlib-surface-surface-intersections.md)
- [Native lamp-well conversion regression](docs/native-lamp-well.md)
- [Rhino oracle setup, Python API, and comparisons](docs/oracle.md)
- [Large-model display mesh diagnostics](docs/display-mesh-diagnostics.md)
