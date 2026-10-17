# Flow implementation work

Flow is registered in the command API and application. Its current supported
subset has a two-guide getter, direction reversal, cached preview, staged edits,
Copy, Rigid/RigidGroups, Stretch, PreserveStructure, option memory and one-step
local Undo/Redo. See [usage and current limits](commands/flow.md).

The implementation owns shared prepared guide geometry and reuses the existing
[arc-length sampler](prepared-curve-length.md), affine direction mapping, morph
fitting and viewport display cages. Straight guide pairs use a shortest-rotation
map. Planar curved guides use their supporting planes; a straight guide borrows
the curved guide's plane. Source points beyond an endpoint retain their tangent
residual separately from mapped guide distance. Stretch scales both distances.
Native coordinates and returned frames are expectations, never construction
inputs for the Rust implementation.

The retained captures use Rhino 8.32.26160.13001 through public RhinoCommon APIs
and actual Flow macros in owned private displays:

| Recipes | Cases | Purpose |
| --- | ---: | --- |
| `flow.json` | 44 | Line, arc, circle, polyline and spatial cubic point maps; both stretch modes and reversals; four actual commands |
| `flow_frames.json` | 18 | Public frame queries and spatial diagnostics |
| `flow_command.json` | 24 | Actual grouped point-cloud commands: line, arc and corner targets × Copy × Rigid × Stretch |
| `flow_planes.json` | 16 | Actual commands and SDK maps across XY/XZ/YZ planes, straight/curved bases and both reversals |
| `flow_equivalence.json` | 19 | Domain changes, exact knot insertion, rotation, scaling, spatial polylines and identity diagnostics |
| `flow_precision.json` | 19 | Point order, SDK precision settings and scale diagnostics |

Of the main 44 maps, 36 match within `1e-6`; eight involving spatial curved
guides remain unresolved. All 24 command and 16 mixed-plane maps match too:
76 qualified point maps in total. Actual command comparisons cover 43 of the
44 retained macros; the spatial command is explicitly rejected locally. Cloud
positions, original/copy identity, source selection, group counts and unchanged,
unselected guides are compared. Native table enumeration order is excluded.
See the [comparison report](flow-command-comparison.json) and
[validation record](flow-command-validation.json).

Spatial differences remain an implementation task, not a tolerance exception.
Independent rotation-minimizing transport does not reproduce the native frames.
Native experiments also show sensitivity to exact knot insertion and small
scaling, while authored point order and the SDK QuickPreview/Tolerance settings
do not change point results. These diagnostic observations are retained to guide
further independent implementation; no spatial parity is claimed.

Remaining scope includes all retained spatial point maps, selected grips, Line
and Local input, Roadlike/Stairlike and TargetSurface orientation, native option
memory and history details, multiple-object rigid group qualification, fitted
owner geometry, planar NURBS and closed-guide qualification, preview appearance and performance.

Replay local maps without launching Rhino:

```sh
cargo run -p viboceros-oracle -- tools/rhino_oracle/fixtures/flow.json /tmp/flow-local.json
cargo run -p viboceros-oracle -- tools/rhino_oracle/fixtures/flow_command.json /tmp/flow-command-local.json
cargo run -p viboceros-oracle -- tools/rhino_oracle/fixtures/flow_planes.json /tmp/flow-planes-local.json
cargo test -p viboceros -p viboceros-command -p viboceros-geometry -p viboceros-oracle flow -- --skip workflow
python3 -m tools.rhino_oracle.flow_report /tmp/flow-local.json /tmp/flow-command-local.json /tmp/flow-planes-local.json --output /tmp/flow-comparison.json
python3 -m unittest tools.rhino_oracle.test_flow_capture
```

Capture fresh native baselines with an owned private Rhino instance and a new
scheme name for each capture:

```sh
python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/flow_command.json --scheme VibocerosOracleFlowCapture --timeout 240 --output /tmp/flow-command-rhino.json
```

References: [Rhino Flow command](https://docs.mcneel.com/rhino/8/help/en-us/commands/flow.htm)
and [public FlowSpaceMorph constructor](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Morphs_FlowSpaceMorph__ctor_1.htm).
Captures inspect no proprietary implementation code.
