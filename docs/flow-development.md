# Flow implementation work

Flow is registered in the command API and application. Its current supported
subset has a two-guide getter, direction reversal, cached preview, staged edits,
Copy, Rigid/RigidGroups, Stretch, PreserveStructure, Local radii, temporary Line guides,
NURBS/composite guides, explicit spatial Roadlike axes, selected control/edit grips, option memory and one-step
local Undo/Redo. See [usage and current limits](commands/flow.md).

The implementation owns shared prepared guide geometry and reuses the existing
[arc-length sampler](prepared-curve-length.md), affine direction mapping, morph
fitting and viewport display cages. Straight guide pairs use a shortest-rotation
map. Opposite directions use the plane determined by the guides' separation;
collinear guides reuse the existing OpenNURBS-style perpendicular policy.
Qualified planar curved guides use their supporting planes; a straight guide borrows
the curved guide's plane. Explicit Roadlike axes provide spatial meridian frames.
Source points beyond an endpoint retain their tangent
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
| `flow_roadlike_poles.json` | 40 | Matching pole/rigid/grip/plane workflows, including the four formerly rejected rational NURBS cases |
| `flow_pole_lookup.json` | 2 | Matching rational-quarter-circle positions and public batch diagnostics |
| `flow_integration.json` | 4 | Forty forward integrals and 32 inverse batches across rational, spatial, refined and composite guides |
| `flow_nurbs.json` | 72 | NURBS and composite base/target/both-guide SDK maps, Stretch and both reversals |
| `flow_nurbs_command.json` | 72 | Complete rational, cubic and composite command workflows with Copy, reversals, groups and Undo/Redo |
| `flow_nurbs_spatial.json` | 60 | Spatial Roadlike base/target/both-guide workflows with Copy, Stretch, reversals, Rigid poses and Undo/Redo |
| `flow_inverse_queries.json`, `flow_inverse_nodes.json` | 4 | Public query order, density, tolerances, curve lifetime and checkpoint snap windows |
| `flow_auto_directions.json` | 38 | Automatic spatial direction under translations, base length, scaling, reversal, refinement, control changes and quarter turns |
| `flow_local.json` | 50 | Local falloff, finite end caps, Copy/Stretch/reversals, straight-guide orientations and a spatial Roadlike workflow |
| `flow_local_owners.json` | 76 | NURBS/composite and spatial guide roles, local Rigid poses, control/edit grips and complete owner/history states |
| `flow_local_edges.json` | 42 | Zero/reversed radii, reflected derivative frames, unchanged whole owners and outside grips |
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
further independent implementation; automatic spatial orientation is not qualified.

Remaining scope includes all retained automatic spatial point maps,
Roadlike Automatic, Stairlike and TargetSurface orientation, native option
memory and history details, multiple-object rigid group qualification, fitted
owner geometry, further closed/periodic guides, preview appearance and performance.

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
kernel. The eight original automatic spatial mismatches and their command guard remain.

Regenerate this analysis without launching Rhino:

```sh
python3 -m tools.rhino_oracle.flow_spatial_report tools/rhino_oracle/fixtures/flow_length_frames.json tools/rhino_oracle/observations/flow_length_frames_rhino_20261016.json /tmp/flow-spatial-report.json
python3 -m unittest tools.rhino_oracle.test_flow_spatial
```

These results required separate implementations of the spatial frame policy
and Flow's position lookup. Replacing the initial frame alone, or merely
loosening the precise shared arc-length sampler, does not match this evidence.
The independent incremental inverse below now supplies the position policy.

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

Remaining automatic spatial work is to generate the directions independently
from authored geometry, qualify further curves and pass all original automatic
spatial maps and command/history workflows. The diagnostic captures alone did
not change the kernel or guard. Explicit spatial axes now use the qualified
incremental lookup and meridian frames described below.

The shared sampler now supplies independent
[`parameters_at_normalized_lengths`](prepared-curve-length.md) batches with
input-order preservation, duplicate reuse and full preflight validation.
The [batch validation record](arc-length-batch-validation.json) retains 14 SDK
cases, including a stationary-endpoint discrepancy checked against an analytic
inverse. This precise geometry path remains separate from Flow's incremental
inverse. The automatic spatial direction selector remains unfinished.

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
convenience. The original eight automatic spatial mismatches and their guard remain.

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

The forty actual pole workflows and the eight earlier
in-plane cases match complete owner/selection/group/history states within
`1e-6`; maximum error is below `2.1e-10`. Four rational-quarter-circle
workflows originally exposed a separate position lookup difference. They now
match through the incremental inverse implementation; the historical rejection
evidence remains available through the validation records' immutable Git hashes.

Two further public SDK captures isolate the position difference independently
of the Roadlike frames. The earlier precise-only kernel centers differed from native Flow by
`5.392740487164538e-5` and `0.0005392740428122877` at unit and ten-unit scales.
Linear interpolation of 841 public batch inverse parameters at fractional
tolerance `1e-4` predicts the five interior/end centers in each case to below
`3e-15`. Along with the spatial cubic captures, this provides a reproducible
lookup target. The independent inverse now generates the table and passes
these two SDK maps too. Native tables remain comparison expectations only.

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
estimates. Four SDK recipes retain a rational quarter circle, a spatial
cubic, its exact knot refinement and a mixed polycurve. Across both guides and five tolerances,
all 40 full-length integrals agree with Rhino; the maximum relative difference
is below `7.26e-15`. Prefix lengths are retained as further diagnostics.

These captures also compare public scalar inverses with batch densities of
2, 16, 100 and 840 intervals. Changing density changes returned midpoint
parameters. Twelve exactly linear runs in the `1e-4`, 840-interval inverse
queries each pass through an independently generated `1e-6` integration
checkpoint, with normalized-distance residuals below `1.6e-12`. This is evidence
for shared checkpoints in native inversion.
Direct linear inversion of the coarse Rust checkpoints differs
from the native 840-interval batch by more than `0.003` in parameter on every
target. Matching forward quadrature alone therefore does not finish inversion.
The new sequential solver tightens each root bracket with checkpoints generated
while integrating that root's prefix, and retains each accepted approximate
point as the next query's origin. Polycurves scale each root's stopping error
by the containing component's integrated length. All 32 retained inverse
batches, comprising 7,696 parameters, match within `1e-8`; observed maximum
parameter error is below `4.3e-12`. No native parameters construct the Rust map.
See [the comparison](../tools/rhino_oracle/observations/flow_integration_report_20261016.json)
and [the sampler API](prepared-curve-length.md).

The public [NormalizedLengthParameters documentation](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Curve_NormalizedLengthParameters_3.htm)
distinguishes a segment's fractional precision from an optional absolute
length constraint. The Flow diagnostics deliberately retain the native
zero-absolute-tolerance batch calls separately from the existing tightly
constrained batch qualification.

The [NURBS validation](flow-nurbs-validation.json) qualifies the resulting Flow
lookup in both directions: 72 default planar SDK maps and 132 complete command
workflows match within `1e-6`. The latter include 60 spatial Roadlike workflows,
both Stretch modes and 12 Rigid poses. Source and target NURBS may appear
independently or together; composite guides, reversals, Copy, group membership,
selection, unchanged guides and real Undo/Redo are retained. Spatial guide
preview uses the same prepared map, with explicit axes picked through the
existing getter. Default spatial Automatic orientation still fails the original
eight witnesses and remains guarded; native Roadlike Automatic is also guarded.

```sh
cargo run -p viboceros-oracle -- tools/rhino_oracle/fixtures/flow_nurbs_command.json /tmp/flow-nurbs-command-local.json
python3 -m tools.rhino_oracle.flow_line_comparison tools/rhino_oracle/fixtures/flow_nurbs_command.json /tmp/flow-nurbs-command-local.json tools/rhino_oracle/observations/flow_nurbs_command_rhino_20261017.json /tmp/flow-nurbs-command-report.json
python3 -m unittest tools.rhino_oracle.test_flow_nurbs tools.rhino_oracle.test_flow_integration
```

The [automatic-direction report](../tools/rhino_oracle/observations/flow_auto_direction_report_20261017.json)
and [capture record](flow-auto-direction-validation.json) isolate orientation
after the lookup implementation. Thirty-eight new SDK maps contain 646 basis
sections. All independently generated Rust centers agree within `5.4e-12`,
while the existing transported transverse bases still differ. A shared fixed
direction fitted on nine sections of each map predicts its eight held-out
sections too; maximum basis-coordinate error is below `7.6e-14`. This analysis
also handles the small-scale case where one homogeneous fit coefficient
vanishes. Fitted directions remain diagnostic expectations only.
Direction errors are norms of cross products of unit directions, so opposite
pole signs are equivalent.

Five base lengths and nine translations leave the target direction unchanged
to below `1.8e-14`; changing the source orientation also leaves it unchanged.
Reversing the target changes it. Scales `0.01` and `0.1` agree with the public
`TryGetPlane(0.1)` normal, while scales `0.2`, `1`, `2` and `10` share the same
nonplanar direction. Exact knot refinement changes that direction by about
`5.4e-6`. Three joint quarter turns fail to rotate the selected direction with
the geometry; rotating the results back gives direction errors above `0.57`.
This rules out a rotation-covariant plane fit alone for these witnesses and
directs further work toward the selector's dependence on world coordinates.
It does not supply that selector. Automatic spatial Flow remains guarded.

```sh
python3 -m tools.rhino_oracle.flow_auto_direction_report /tmp/flow-auto-direction-report.json
python3 -m unittest tools.rhino_oracle.test_flow_auto_direction
```

The [Local validation record](flow-local-validation.json) qualifies source
distance falloff independently of the frame selector. The kernel reuses the
same prepared guide and closest-point query; the clamped cubic blend is the
one used by the open-source OpenNURBS `ON_Localizer`. Distance includes the
full endpoint residual, giving finite end caps. Points outside return before
evaluating a potentially unrepresentable full deformation. Convex coordinate
blending reuses the shared scale-safe scalar interpolator.

Both radius orders and a zero first radius match actual native commands. A
typed radius pair normalizes into validated inner/outer bounds. Interactive
input reuses the existing Circle getter, drafting plane, snapping and shared
radius-circle painting. Changing radii retains the prepared guide data; preview
and commit use the same point map. Partial, invalid or cancelled getters do not
edit the document. New commands retain the Local toggle but ask for radii again.

Local exposed two existing edge cases. A reflected numerical derivative can
cancel one averaged rigid-frame axis exactly. Retaining its forward direction
while averaging the other axis matches native poses, including all three world
axis permutations. The shared rigid producer now handles this case. Outside
grips are filtered without changing the original grip-owner precedence or
ordinary rigid-group context. Native skips their unchanged owner, while whole
objects still receive Copy even when unchanged. All 168 retained workflows
compare complete owner, selection, group, guide and real Undo/Redo states.
Further fitted owner geometry, mixed rigid groups and automatic spatial
orientation still require qualification.

```sh
cargo run -p viboceros-oracle -- tools/rhino_oracle/fixtures/flow_local_owners.json /tmp/flow-local-owners.json
python3 -m tools.rhino_oracle.flow_line_comparison tools/rhino_oracle/fixtures/flow_local_owners.json /tmp/flow-local-owners.json tools/rhino_oracle/observations/flow_local_owners_rhino_20261017.json /tmp/flow-local-owners-report.json
python3 -m unittest tools.rhino_oracle.test_flow_local
```


The [TargetSurface validation record](flow-target-surface-validation.json)
qualifies 119 actual Flow commands using authored planes, rational cylinders
and a curved rational tensor-product patch. The target guide keeps its existing
arc-length lookup. Each section queries the shared surface closest-point and
regular-normal kernels, respecting B-rep face reversal. Surface orientation
supersedes the target Roadlike axis. It allows spatial target guides without
using the still-unqualified automatic spatial frame selector. Spatial source
guides still require explicit Roadlike axes.

The command and preview capture immutable surface/face references using the
existing surface resolver. They share the point map, owner/grip staging,
groups and one-step Undo/Redo. Native surface picking releases source object
and grip selection; final history states follow that behavior. Hidden, locked,
replaced and removed references invalidate preview without deforming sources.
Native Local/Rigid rational-curve witnesses exposed a center error from using
model-tolerance bounds for a placement pose. Flow now refines the existing
bound kernel with numerical validation tolerance before choosing rigid centers
in both preview and commit.
No native coordinates, UV values or fitted directions initialize the kernel.

```sh
cargo run -p viboceros-oracle -- tools/rhino_oracle/fixtures/flow_target_surface.json /tmp/flow-target-surface.json
python3 -m tools.rhino_oracle.flow_line_comparison tools/rhino_oracle/fixtures/flow_target_surface.json /tmp/flow-target-surface.json tools/rhino_oracle/observations/flow_target_surface_rhino_20261017.json /tmp/flow-target-surface-report.json
python3 -m unittest tools.rhino_oracle.test_flow_target_surface
```


The patch witnesses also distinguish unstretched lookup preparation. When a
nonlinear NURBS target is longer than the base, Flow uses its source-length
prefix: independently invert that fraction with the existing incremental
policy, trim a private copy, and prepare the prefix's 841-point inverse table.
Retain the original guide length and geometry, and normalize the prefix table
by the base length. Clipping a full-curve parameter array, a precise cut, or
merely scaling its fractions gives different results. Two retained public SDK
maps compare this case and a longer curved base; a native corner command checks
the prefix endpoint. Surface normals are queried at the full axial destination,
including the source endpoint residual, before transverse offsets are applied.
