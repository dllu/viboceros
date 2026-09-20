# Align

[Command reference](README.md) · [Transforms](transforms.md) · [Distribute](distribute.md)

`Align` translates selected objects or rigid group units by their tight bounding
boxes. It supports `Left`, `Right`, `Top`, `Bottom`, `HorizCenter`, `VertCenter`,
and `Concentric`, plus `ToLine`, `ToPlane`, `ToFitPlane` and `ToCurve` alignment, using
`AlignTo=CPlane` (initial default) or `AlignTo=World`.
The coordinate choice is remembered by the command registry, outside undo
history; each invocation asks for its alignment mode.

| Mode | Moved coordinates | Automatic destination |
| --- | --- | --- |
| Left / Right | X | Overall minimum / maximum X |
| Bottom / Top | Y | Overall minimum / maximum Y |
| HorizCenter | Y | Overall Y midpoint |
| VertCenter | X | Overall X midpoint |
| Concentric | X and Y | Overall XY midpoint |

The center names describe the resulting alignment line: `HorizCenter` moves
along Y, not X. The coordinate system's Z component is preserved, including
when the target point is off-plane. These axis meanings and default targets
were checked with the actual Rhino 8.32 command, not inferred from option names.
See also [McNeel's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/align.htm).

## Projection alignment

```text
Align ToLine 1,2,3 5,7,11
Align ToPlane AlignTo=World 1,2,3 5,7,11
Align ToPlane 3Point 1,2,3 5,7,11 -2,3,5
Align ToFitPlane AlignTo=World
Align ToCurve CurveId=<object-uuid>
```

These modes translate **each selected object independently**, including grouped
objects. They do not flatten or otherwise deform the geometry. The anchor is
the object's tight-box bottom center `(midX, midY, minZ)` in the chosen
coordinate system, not its three-dimensional box center. Therefore `AlignTo`
affects nonpoint anchors even when the target line or three-point plane is the
same in world coordinates. Group memberships and unselected peers are unchanged.

- `ToLine` orthogonally projects the anchor onto the infinite supporting line
  through the two reference points; it does not clamp to their segment.
- Two-point `ToPlane` uses the plane through the first reference containing
  their connecting line and parallel to the chosen system's Z axis. Equivalently,
  its normal is `(end - start) × Z`. Translation preserves that Z component.
- `ToPlane 3Point` uses the plane through three noncollinear world points.
  The target plane itself is independent of `AlignTo`.
- `ToFitPlane` fits a least-squares plane to the individual bottom-center
  anchors of at least three selected objects. It needs no reference picks.
  See [best-fit plane design and evidence](../plane-fit.md).
- `ToCurve` moves each anchor to a closest point on one **unselected** target
  curve. Finite lines and open curves clamp to their ends. The target is read
  without joining or changing the source selection, including when grouped
  with a source. Lines, arcs, circles, ellipses, NURBS, polylines and polycurves
  retain their native parameterization and the target geometry is unchanged.

After choosing `ToCurve`, click a curve or enter `CurveId=<object-uuid>`.
This is an object-picking phase, not a drafting-point prompt: point input and
Enter/Auto do not complete it; an invalid hit leaves the source set intact.
Window selection is ignored during this single-target phase. Esc cancels,
and another alignment mode discards the pending target. UUID input is the
native scripting extension; the Rhino oracle resolves fixture indices into
owned Rhino IDs and uses the public `SelID` selection action.

The app stages two or three picked/typed points without changing the model.
Incomplete point sets cannot finish with Enter/Auto. Invalid final points leave
the previous points available for correction. Changing mode discards staged
references; cancellation discards the entire pending edit. Fully typed calls
execute directly. `3Point=Yes|No` is also accepted for typed selection options.

## Input and document behavior

```text
Align Left Auto
Align Concentric AlignTo=World 20,-4,17
Align HorizCenter AlignTo=CPlane
```

In the app, bare `Align` asks for objects when none are preselected, then an
alignment mode and a target point. Enter at the point prompt uses automatic
alignment; `Auto` also completes without a pick. A fully typed `Auto` invocation
executes immediately. A command-registry call without a target uses automatic
alignment for the seven bounding-box modes. Explicit complete-command point arguments are world coordinates;
the interactive point prompt uses the shared CPlane/world/relative coordinate
input, including `w20,-4,17` for an explicit world point.

For the seven bounding-box modes, objects in the same last group membership move together. Overlapping earlier
memberships do not recursively combine units, and unselected members do not
move. Normal grouped viewport selection may select peers; the command's unit
builder only consumes the selection it receives. Partial selections made by
Rhino's `SelID` prompt action remain partial in the oracle fixtures.

Preselected objects remain selected. Objects selected after starting `Align`
are deselected after success, and after a terminal `ToFitPlane` failure caused
by fewer than three selected objects. Both paths preserve IDs, attributes, layers,
ordered group memberships, and source geometry types. Actual geometry changes
form one undo step. A no-op creates no model-history entry; postselection still
clears selection. Invalid options, failed bounds, and unrepresentable
translations leave all model geometry unchanged. Failed interactive target
queries stay open for correction; Esc cancels without applying an edit.

## Implementation and limits

The command's `align/options` module supplies the shared command/UI parser.
`layout_units` owns the selected rigid units used by both Align and Distribute.
`object_bounds` supplies tolerance-controlled bounds in a geometry-local frame;
the CPlane display origin does not determine the arithmetic origin. Bounds are
computed for the locus, including trimmed faces, not from control polygons or
display meshes. Every replacement is staged before document mutation.
The existing [tight-bound limits](../trimmed-face-bounds.md) apply: poles,
unresolved bounds, and finite-range failures are errors, not approximate output.
The working geometry, projected target, displacement, and translated output
must remain representable in binary64.

Projection modes use a separate per-object path. Lines and planes use the kernel's
prepared `PointProjection3`. Line differences and plane normals are defined with exact
binary64-input arithmetic; oblique queries round once per output coordinate.
Axis-aligned queries use direct coordinate replacement. The kernel accepts
exactly distinct/noncollinear definitions without an implicit distance tolerance.
The Rhino batch worker conservatively rejects tiny/near-degenerate references
before executing, to prevent an incomplete interactive macro. Rhino's precise
near-degeneracy prompt thresholds have not been matched.

`cargo run --release -p viboceros-geometry --example profile_point_projection`
measures prepared query cost, excluding construction. One DGX Spark release run
measured approximately 3.6 ns for axis-aligned lines, 13–14 µs for ordinary
oblique line/plane queries, and 143 µs for a wide-exponent cancellation case.
These are local measurements, not a Rhino speed comparison. Optimizing the
exact oblique path and establishing command-level performance parity remain open.

`ToCurve` uses the kernel's native closest-parameter dispatch. Ellipses use a
[principal-quadrant root](../ellipse-closest-point.md); polyline/polycurve
segments and arc endpoints use exact distance ordering of evaluated points.
General [NURBS closest-point search](../curve-closest-point.md) is bounded,
not a certified global minimum. Finite-range and extreme coefficient failures
remain explicit errors. Curve-alignment performance parity with Rhino is unmeasured.

Other plane-creation options, control-point/grip editing, and SubD
component alignment are not implemented. They are not aliases for bounding-box
translation. No preview of the prospective transformed geometry is provided.

## Oracle evidence

The [48-case fixture](../../tools/rhino_oracle/fixtures/align.json) has a
[Rhino 8.32 record](../../tools/rhino_oracle/observations/align.json). It covers
all seven modes, automatic/picked targets, world/rotated/oblique CPlanes,
point clouds, single points, rational and signed curves, analytic conics,
polylines/polycurves, trimmed B-reps, overlapping groups and partial selection.
Four [mixed-geometry cases](../../tools/rhino_oracle/fixtures/align_mixed.json)
have a [separate record](../../tools/rhino_oracle/observations/align_mixed.json)
covering triangle/quad meshes, planar surfaces and lines.
The [four postselection cases](../../tools/rhino_oracle/fixtures/align_postselection.json)
have a [separate record](../../tools/rhino_oracle/observations/align_postselection.json)
checking actual command-first selection, including cleanup and partial groups.
These probes run in an owned private Xvfb session and restore their objects,
groups, selection and construction planes. They are untimed command probes.

Acceptance for these fixtures is absolute `1e-8`, relative `1e-12`; the largest
observed passing coordinate difference is below `7.5e-10`. Native tests also
check independent box equations, a quadratic arch's true extremum, distant
display-plane origins, undo/redo, late failures, and interactive phase handling.
The [final executable comparison](../align-comparison.json) retains executable
and fixture hashes, per-case errors, 168 passing operations and all 13 failing
diagnostics. No cases are dropped from the combined evidence.

The [34 projection cases](../../tools/rhino_oracle/fixtures/align_projection.json)
have a [Rhino record](../../tools/rhino_oracle/observations/align_projection.json)
covering both coordinate systems, grouped and postselected objects, three-point
planes, clouds, lines, rational/signed curves, conics, polylines/polycurves,
planar surfaces and exactly representable mesh translations. The largest
projection-case difference is below `6e-10`.

Four [mesh precision diagnostics](../../tools/rhino_oracle/fixtures/align_projection_mesh_diagnostics.json)
retain their [raw Rhino records](../../tools/rhino_oracle/observations/align_projection_mesh_diagnostics.json)
and fail the same `1e-8` comparison, with a maximum difference of about `1.05e-7`.
Every mesh coordinate matches exactly after conversion of the native value to
binary32; every nonmesh value and document-state field is also checked. This
separate storage diagnostic does not round native geometry or alter the raw
comparison results.

[Best-fit plane evidence](../plane-fit.md#evidence-and-remaining-differences)
adds successful and terminal-failure cases, plus six explicit mesh, nonunique
normal, and curved-bound diagnostics at the same comparison threshold.

The [39 curve cases](../../tools/rhino_oracle/fixtures/align_curve.json) have
[completed Rhino observations](../../tools/rhino_oracle/observations/align_curve.json)
covering all seven target families, World/CPlane, pre/postselection, groups,
oblique circles, rational 3D curves, closed polylines, ties and no-ops. Every
geometry/domain/document-state field passes the unchanged threshold; maximum
coordinate difference is below `2.4e-14`. Independent native tests cover finite
endpoint clamping, bottom-center anchors, undo/redo and atomic late failures.
The [selected-target diagnostic](../../tools/rhino_oracle/observations/align_curve_target_diagnostic.json)
records a separate timed-out Rhino macro: `SelID` cannot finish the curve prompt
when it names an already selected source. The adapter rejects such fixtures;
the native command conservatively requires an unselected target. This timeout
is not counted as a geometry observation or full mouse-picking parity.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/align.json --absolute-epsilon 1e-8 --relative-epsilon 1e-12 --timeout 300
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/align_postselection.json --absolute-epsilon 1e-8 --relative-epsilon 1e-12 --timeout 240
```

Three cases remain explicitly failing in
[`align_diagnostics.json`](../../tools/rhino_oracle/fixtures/align_diagnostics.json),
with an unchanged [Rhino record](../../tools/rhino_oracle/observations/align_diagnostics.json).
For oblique quadratic-surface Right/Concentric alignment, maximum coordinate
differences are about `0.000559` / `0.000280`; for paraboloid-disk Concentric
alignment the difference is about `0.000179`. The same underlying discrepancy
is recorded for [Distribute](distribute.md). Analytic extrema independently
validate the native translations:

- On the square patch, the unnormalized X projection is
  `13u-12u²+26v-24v²`, with maximum `169/16` at `u=v=13/24`.
- On the radius-0.8 disk, it is `u+2v+3(u²+v²)`, with minimum `-5/12` and maximum
  `3(0.8)²+0.8√5`.

The regression tests check these translations against the formulas and retain
the measured disagreements; the comparison epsilon is not widened to absorb
Rhino's less accurate bounds. Full command parity is not claimed.

The README remains a short build/run guide; detailed command behavior and
numerical evidence are maintained here and in the linked kernel notes.
