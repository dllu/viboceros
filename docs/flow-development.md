# Flow implementation work

Flow is registered in the command API and application. Its current supported
subset has a two-guide getter, direction reversal, cached preview, staged edits,
Copy, Rigid/RigidGroups, Stretch, PreserveStructure, temporary Line guides, selected control/edit grips, option memory and one-step
local Undo/Redo. See [usage and current limits](commands/flow.md).

The implementation owns shared prepared guide geometry and reuses the existing
[arc-length sampler](prepared-curve-length.md), affine direction mapping, morph
fitting and viewport display cages. Straight guide pairs use a shortest-rotation
map. Opposite directions use the plane determined by the guides' separation;
collinear guides reuse the existing OpenNURBS-style perpendicular policy.
Planar curved guides use their supporting planes; a straight guide borrows
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
| `flow_grips.json` | 72 | Complete grip owner, group, selection and Undo/Redo workflows across curves, surfaces and meshes |
| `flow_line.json` | 128 | Actual commands with either or both guides drawn, Copy/Rigid/Stretch, reversals, mixed planes, selected grips and rigid curve owners |
| `flow_line_orientation.json` | 24 | Twenty actual commands and four SDK diagnostics for opposite, collinear and nearly opposite lines |

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

Remaining scope includes all retained spatial point maps, Local
input, Roadlike/Stairlike and TargetSurface orientation, native option
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

The [grip qualification](flow-grips-validation.json) additionally compares 72
actual commands, including mixed ordinary objects and grip owners. Grips always
follow the point map; Rigid/RigidGroups govern ordinary peers. Copied grip owners
stay in source groups. Reuse the document's shared grip reconstruction and morph
copy paths while accepting prepared ordinary geometries, so the complete mixed
edit stages before geometry changes. The existing display cache previews the
same grip basis and group poses without editing the document.

The native grip driver runs at Rhino idle outside RunPythonScript's undo record.
It records EndCommand, the completed macro, Undo and Redo separately. Temporary
guide selection and postselection cleanup still appear at EndCommand; comparisons
use the completed macro's state. Complete macros have no trailing Cancel token.
Earlier discovery captures inside the script's undo record or with an extra
Cancel are not qualified. Further grip-family, seam/alias and preview appearance
qualification remains open.

Line input uses the ordinary drafting point getter, anchors, filters and snaps.
Both guide roles accept an immutable pair of endpoints through the shared
command/preview preparation path; no temporary document object is created.
Interactive preview and cancelled or invalid pairs leave the model and history
unchanged. Scripted input accepts `BaseLine`, `TargetLine` or `Line` followed by
two points. See the [Line validation record](flow-line-validation.json).

The 128-command capture merges a 96-case matrix and a separate 32-case parallel
matrix without changing numeric outputs. An additional 24-case orientation
capture retains all collinear, separated and near-opposite diagnostics. These
showed that separated opposite guides need a half-turn axis perpendicular to
their common direction and separation. Collinear guides use the existing
perpendicular helper. Any nonzero cross product between nearly opposite
directions retains its unique axis, even below the usual angular tolerance.
The kernel regression includes finite results when subtracting large guide
origins would otherwise overflow. All 152 retained Line observations match
within `1e-6`; maximum coordinate error is `8.341061175087816e-11`.
