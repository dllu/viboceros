# Flow implementation work

Flow is registered in the command API and application. Its current supported
subset has a two-guide getter, direction reversal, cached preview, staged edits,
Copy, Rigid/RigidGroups, Stretch, PreserveStructure, temporary Line guides, explicit Roadlike axes, selected control/edit grips, option memory and one-step
local Undo/Redo. See [usage and current limits](commands/flow.md).

The implementation owns shared prepared guide geometry and reuses the existing
[arc-length sampler](prepared-curve-length.md), affine direction mapping, morph
fitting and viewport display cages. Straight guide pairs use a shortest-rotation
map. Opposite directions use the plane determined by the guides' separation;
collinear guides reuse the existing OpenNURBS-style perpendicular policy.
Qualified planar curved guides use their supporting planes; a straight guide borrows
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
| `flow_roadlike.json` | 40 | Explicit Roadlike axes and CPlane-Z picker presets, selected/drawn guides, reversals, Copy/Rigid/Stretch, rational control/edit grips and full Undo/Redo |
| `flow_roadlike_diagnostics.json` | 12 | Eight formerly blocked in-plane cases now match; four native Automatic cases remain rejected |
| `flow_roadlike_poles.json` | 40 | Thirty-six matching pole/rigid/grip/plane workflows and four retained curved-NURBS lookup rejections |
| `flow_pole_lookup.json` | 2 | Precise-kernel versus native positions and public 840-interval batch diagnostics for two rational quarter circles |
| `flow_integration.json` | 3 | Five-tolerance forward integration and prefix lengths, scalar inverses and four batch densities for rational and spatial NURBS |
| `flow_length_frames.json` | 6 | Public length/frame and tolerance-classification queries for a spatial cubic, exact refinement, rotation and three scales |
| `flow_density.json`, `flow_coupled_frames.json`, `flow_up_lookup.json` | 13 | Dense spatial positions/bases, equivalent straight guide types, batch length lookups, degree elevation and changed spatial shapes |

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
input, Roadlike Automatic, native curved-NURBS lookups, Stairlike and TargetSurface orientation, native option
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

The [spatial diagnostic report](../tools/rhino_oracle/observations/flow_spatial_report_20261016.json)
and [capture record](flow-spatial-validation.json) retain 101 normalized-length
queries on both guides of six recipes. Their SDK point maps exactly reproduce
the earlier captures. On the dense cubic witness, native Flow centers differ
from the precise public `NormalizedLengthParameter` query by up to
`0.00012836943112159993`. Public fractional length tolerances of `1e-2`, `1e-4`,
`1e-6` and `1e-8` also fail to reproduce the Flow centers. A simple change to
the arc-length solver's tolerance is therefore insufficient for this witness.

At corresponding normalized lengths, the single and batch public perpendicular
frame queries differ from Flow by up to `15.49` degrees after aligning their
initial roll. Two small-scale initial frame modes are reproduced by frames
constructed from the public `TryGetPlane(0.1)` result. This is diagnostic evidence
for a classification rule, not qualification of a complete spatial policy.
Neither returned native frames nor candidate classification normals feed the
kernel. The eight original spatial mismatches and command guard remain.

Regenerate this analysis without launching Rhino:

```sh
python3 -m tools.rhino_oracle.flow_spatial_report tools/rhino_oracle/fixtures/flow_length_frames.json tools/rhino_oracle/observations/flow_length_frames_rhino_20261016.json /tmp/flow-spatial-report.json
python3 -m unittest tools.rhino_oracle.test_flow_spatial
```

The next implementation work must account for both the spatial frame policy
and Flow's position lookup. Replacing the initial frame alone, or merely
loosening the precise shared arc-length sampler, does not match this evidence.

The [structure report](../tools/rhino_oracle/observations/flow_structure_report_20261016.json)
and [capture record](flow-structure-validation.json) isolate two further rules.
For the retained cubic, linear interpolation of 841 public
`NormalizedLengthParameters` values at fractional tolerance `1e-4` predicts
2,048 native Flow centers and 682 additional centers to below `1e-14`.
The same table predicts 2,048 centers on the exact degree-elevated curve to
below `3e-13`. A 100-interval public batch does not match. This establishes an
840-interval lookup for this geometry, not a universal subdivision count.

For three spatial shapes, a shared fixed direction projected into the base
and target tangent-normal planes reproduces the native transverse bases to
below `2e-13`. Directions fitted on 17 of the dense witness's 682 sections
also predict the other 665 sections; the two changed shapes use nine fitted
and eight held-out sections each. The fitted direction varies by shape.
It is diagnostic evidence for the frame family, not a geometry-derived
selection algorithm. None of these fitted directions or native inverse tables
initialize Rust morphs. The six equivalent line/NURBS/polyline guide pairs
agree to below `4e-15`, ruling out that representation difference in these cases.

The optional structure analysis uses NumPy for SVD; the core Python oracle
and its ordinary checks continue to use the standard library.

```sh
python3 -m tools.rhino_oracle.flow_structure_report /tmp/flow-structure-report.json
python3 -m unittest tools.rhino_oracle.test_flow_structure
```

Remaining spatial work is to generate the automatic directions and bulk inverse
parameters independently from authored geometry, qualify the subdivision policy
on further curves and directions, and pass all original spatial maps and actual
command/history workflows. The production kernel and spatial command guard
remain unchanged by this diagnostic work.

The shared sampler now supplies independent
[`parameters_at_normalized_lengths`](prepared-curve-length.md) batches with
input-order preservation, duplicate reuse and full preflight validation.
The [batch validation record](arc-length-batch-validation.json) retains 14 SDK
cases, including a stationary-endpoint discrepancy checked against an analytic
inverse. This is the precise geometry path needed to construct lookup tables;
the native coarse inverse approximation and automatic spatial direction selector
remain implementation work.

Explicit Roadlike axes now use the existing meridian-frame kernel, prepared
length sampler, owner/grip reconstruction and preview cache. The input session
collects two points for each axis, accepts the active CPlane Z as an explicit
picker preset, and rebuilds the cached morph when either axis changes. The
registered command requires both world directions and preflights the supported
guide families before editing.

The [Roadlike validation record](flow-roadlike-validation.json) retains 52 actual
native commands in one owned capture. Forty explicit-axis workflows match full
owner geometry, guide lifetime, groups, selection and Undo/Redo within `1e-6`;
maximum coordinate error is `5.7063687108893646e-11`. Four of these use rational
control/edit grips and four choose explicit directions from nonstandard CPlanes.
The native macro for a CPlane preset picks the corresponding world direction;
it does not invoke native Automatic. The remaining twelve observations retain
eight in-plane-axis cases and four actual native Automatic cases. The eight
in-plane cases now match through the pole implementation below. Native
Automatic remains explicitly rejected without a document or history edit.
All original native observations remain retained. Native
Automatic's selection rule remains unresolved; CPlane Z is a separate picker
convenience. The original eight spatial mismatches and command guard remain.

Replay the new supported and diagnostic cases without launching Rhino:

```sh
cargo run -p viboceros-oracle -- tools/rhino_oracle/fixtures/flow_roadlike.json /tmp/flow-roadlike-local.json
cargo test -p viboceros-oracle --lib flow_roadlike
python3 -m unittest tools.rhino_oracle.test_flow_roadlike
```

The [pole validation record](flow-poles-validation.json) extends the section
model to represent a rank-one map at an exact Roadlike pole. `Frame3` remains
orthonormal; a separate internal Flow section carries zero transverse vectors.
Nonzero cross products keep their direction, including very small ones. Rigid
returns the original pose when a Roadlike placement frame is degenerate;
nonfinite and other failures still propagate.

Three-point arcs reuse the public OpenNURBS constructor after tolerance checks,
with the scaled Rust construction retained for inputs outside its numerical
range. Arc evaluation uses the retained supporting plane/angular interval,
including reversal, and the public OpenNURBS quadrant-cleanup rule. The native
build disables multiply/add contraction to preserve those double-operation
boundaries across ARM and x86. This resolved a small supporting-plane tilt that
otherwise selected a different side of a pole. No Rhino outputs initialize the
production kernel.

Thirty-six of forty new actual command workflows and the eight earlier
in-plane cases match complete owner/selection/group/history states within
`1e-6`; maximum error is below `2.1e-10`. Four new rational-quarter-circle
workflows expose a separate position lookup difference. Curved NURBS rails,
including polycurve segments, are now rejected before editing in both ordinary
and Roadlike modes. All four native successes and local rejections remain
visible; this is an outstanding implementation task, not a tolerance exception.

Two further public SDK captures isolate the position difference independently
of the Roadlike frames. Precise kernel centers differ from native Flow by
`5.392740487164538e-5` and `0.0005392740428122877` at unit and ten-unit scales.
Linear interpolation of 841 public batch inverse parameters at fractional
tolerance `1e-4` predicts the five interior/end centers in each case to below
`3e-15`. Along with the spatial cubic captures, this provides a reproducible
lookup target. The native tables are diagnostic expectations only; generating
the coarse inverse and establishing the subdivision rule remain open.

```sh
python3 -m tools.rhino_oracle.flow_pole_report /tmp/flow-poles-report.json
python3 -m unittest tools.rhino_oracle.test_flow_roadlike
```

Public sources used for the reusable arc behavior:
[OpenNURBS arc-curve evaluator](https://github.com/mcneel/opennurbs/blob/8.x/opennurbs_arccurve.cpp),
[three-point circle construction](https://github.com/mcneel/opennurbs/blob/8.x/opennurbs_circle.cpp),
and [Rhino3dm's arc wrapper](https://github.com/mcneel/rhino3dm/blob/main/src/librhino3dm_native/on_arc.cpp).

The [integration record](flow-integration-validation.json) isolates the forward
quadrature stage. The shared sampler now exposes accepted Cash–Karp 5/4
checkpoints, normalized speed error control, span boundaries and local error
estimates. Three fresh SDK recipes retain a rational quarter circle, a spatial
cubic and its exact knot refinement. Across both guides and five tolerances,
all 30 full-length integrals agree with Rhino; the maximum relative difference
is below `7.26e-15`. Prefix lengths are retained as further diagnostics.

These captures also compare public scalar inverses with batch densities of
2, 16, 100 and 840 intervals. Changing density changes returned midpoint
parameters. Twelve exactly linear runs in the `1e-4`, 840-interval inverse
queries each pass through an independently generated `1e-6` integration
checkpoint, with normalized-distance residuals below `1.6e-12`. This is evidence
for shared checkpoints in native inversion; the refinement rule remains open.
Direct linear inversion of the coarse Rust checkpoints differs
from the native 840-interval batch by more than `0.003` in parameter on every
target. Matching forward quadrature therefore does not finish native inversion.
No native parameters initialize the Rust trace, and curved NURBS guide guards
remain until the inverse and frame policies are implemented and qualified.
See [the comparison](../tools/rhino_oracle/observations/flow_integration_report_20261016.json)
and [the sampler API](prepared-curve-length.md).

The public [NormalizedLengthParameters documentation](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Curve_NormalizedLengthParameters_3.htm)
distinguishes a segment's fractional precision from an optional absolute
length constraint. The Flow diagnostics deliberately retain the native
zero-absolute-tolerance batch calls separately from the existing tightly
constrained batch qualification.
