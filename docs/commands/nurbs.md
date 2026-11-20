# NURBS structure and parameterization

[Command reference](README.md) · [Project overview](../../README.md)

For independent U/V control counts and degrees, see [surface Rebuild](rebuild-surfaces.md).

## Degree, knots, and control points

`ChangeDegree degree|u_degree,v_degree Deformable=Yes|No` changes selected
curves and untrimmed NURBS surfaces in place and defaults to `Deformable=No`.
A scalar applies to curves and both surface directions; with a pair, the first
degree also applies to curves. Non-deformable elevation preserves exact
geometry and parameterization while raising knot multiplicities. Lowering, or
`Deformable=Yes`, keeps distinct knot breaks as simple knots and interpolates
the source in homogeneous space at the new Greville abscissae. Degree changes
support values 1 through 11 and can turn periodic directions into clamped
ones, matching Rhino.

Viboceros uses native elevation and homogeneous interpolation
for every mode; see [qualification and limits](../smlib-degree-change.md).

`MakeUniform` replaces selected curves and untrimmed NURBS surfaces in place
with the same degree, control locations, and rational weights but
Rhino-compatible unit-spaced knots. Start and end clamping are retained
independently, periodic topology stays periodic, and supported analytic curves
are first converted to their exact NURBS form. Both surface directions are
changed. `MakeUniformUV Direction=U|V` performs the same operation on selected
untrimmed NURBS surfaces in one direction only and defaults to U. Changing knot
spacing can change the object shape.

`MakePeriodic Smooth=Yes|No DeleteInput=Yes|No` converts selected closed
degree-two-or-higher curves and one eligible closed direction of each selected
untrimmed NURBS surface in place; smoothing defaults to Yes. U is chosen first
when both surface directions are eligible, so running the command again
converts V. `Smooth=Yes` retains active knot breaks and solves the seam in
homogeneous space, while `Smooth=No` retains existing controls as closely as
Rhino permits and redistributes seam knots. Active domains are preserved, and
rational inputs are solved without discarding their weights. `DeleteInput=Yes`
replaces inputs in place by default; No retains the selected sources and adds
unselected copies with their attributes and group memberships. Both modes
support undo.

`MakeNonPeriodic` converts every selected periodic NURBS curve or surface to
the equivalent clamped form in place. It preserves the active domains,
parameterization, shape, object identity, attributes, and selection; surfaces
are clamped in every periodic direction.

The command shares the usd-brep endpoint-split adapter for this conversion. See [qualification and limits](../smlib-nonperiodic.md).

[InsertControlPoint](insert-control-point.md) provides repeated shape-changing
picks, Midpoint placement, surface-net extension and transactional previews.
Its surface Direction names the row orientation and accepts U or V.

Interactive [InsertKnot](insert-knot.md) supports repeated point and curve-parameter
input, Automatic, local Undo and temporary portions on curves and single surfaces.
Its surface Direction names the displayed isocurve.

The explicit `InsertKnot parameter|u,v` form requires one selected curve or
untrimmed NURBS surface and refines it in place without changing its
parameterization or shape. Curves take
one parameter; surfaces take `u,v`, with a scalar accepted when `Direction=U`
or `V`. `Multiplicity` is the target knot multiplicity and may range from one
through the relevant degree. `Symmetrical=Yes` also inserts the parameter
mirrored across the active domain. Rational refinement and periodic repair
follow Rhino/OpenNURBS behavior; identity, attributes, selection, and undo are
preserved.

In this explicit form, Direction names the parameter axis being refined.

Viboceros uses usd-brep for curve/tensor knot refinement.
See [native behavior, qualification and limits](../smlib-knot-refinement.md).

For repeated picking, shape previews, local Undo, temporary curves and periodic
directions, see [interactive RemoveKnot](remove-knot.md). The legacy numeric form
`RemoveKnot parameter|u,v Direction=U|V` requires exactly one selected curve
or untrimmed NURBS surface and removes one knot in place. On curves, the
parameter identifies a curve point and the knot with the nearest model-space
point is selected, matching Rhino. On surfaces, U is the default and the knot
value nearest the chosen coordinate is removed consistently across the whole
control net. Remaining homogeneous controls are interpolated at Greville
abscissae; rational weights, object identity, attributes, selection, and undo
are preserved. Periodic directions are rejected, while non-clamped directions
are first clamped without changing their active domain.

Viboceros uses the shared native homogeneous fitting adapter;
see [qualification and limits](../smlib-knot-removal.md).

For viewport control and row picking, see [RemoveControlPoint](remove-control-point.md).
Its single-pick command keeps rational weights and supports ordinary undo.
The legacy grip-deletion API `RemoveControlPoint index Direction=U|V` requires exactly one selected curve
or untrimmed NURBS surface and removes a zero-based control-point grip in
place. On a surface, the complete row at that index is removed in the chosen
direction, which defaults to U. Remaining controls are retained, with
Rhino-compatible knot updates, endpoint-weight normalization, single-span
degree lowering, and periodic topology repair. The operation generally changes
shape while preserving object identity, attributes, selection, and undo.

`RemoveMultiKnot RemoveFullyMultipleKnots=Yes|No MaxKinkAngle=0..180`
reduces qualifying stacked knots on every selected curve and untrimmed NURBS
surface. By default, only non-full multiple knots are collapsed to simple
knots. Enabling full removal also merges kinks or surface creases whose tangent
angle is strictly below `MaxKinkAngle` in degrees (default 1 degree),
and merges eligible degree-one spans into a single linear span. Surface U and V
directions are both processed using Rhino/OpenNURBS continuity samples.
Rational weights, object identity, attributes, selection, and one-step undo
are preserved; periodic directions are rejected atomically. Non-clamped inputs
are first clamped to their existing active domains so the output knot vectors
remain valid.

The command delegates ordered group refits to the shared native interpolator. See [qualification and limits](../smlib-multiple-knot-removal.md).

## Exact conversion

[`ToNURBS`](to-nurbs.md) exactly converts curves and triangle/quad meshes. Copies
retain source attributes and groups; replacement retains source identity and
renews chronological object order. Native curve intervals and polyline vertex
parameters are preserved. Existing NURBS geometry and ellipses are no-ops.
Deletion and triangle-trimming options are remembered only after real conversions.

## Span decomposition

[`ConvertToSingleSpans`](single-spans.md) decomposes selected surfaces and
single-face B-reps in `Direction=U`, `V`, or `Both`. Outputs have fresh current-layer
attributes and no groups; requested axes acquire `[0,1]` domains. Inputs already
single-span in the requested direction(s) are left entirely unchanged. U/V
`Toggle` and [remembered options](../command-options.md) are supported.

[`ConvertToBeziers`](beziers.md) exactly decomposes curves and underlying surfaces
into fresh, ungrouped, unselected pieces on the current layer. Every output
domain is `[0,1]` (in both directions for surfaces); trims are discarded.
`DeleteInput=Yes` deletes eligible sources while retaining empty group definitions.

## Seams and parameter domains

Run `SrfSeam` on one closed surface, or select it at the prompt. The viewport
shows the current seam and previews the new one as you move the pointer. On doubly
closed surfaces, choose `Direction=U`, `Direction=V`, or `Direction=Both`.
U and V name the direction of the seam curve. `Toggle` switches between U and V.
Click the surface or enter a point to apply the change, or press Esc to cancel.
The interactive edit keeps the original U and V domains and the orientation of
natural single-face B-reps.

The direct form `SrfSeam point Direction=U|V|Both` names the parameter axes to
relocate. `Parameter=value` supplies one axis directly, and `Parameter=u,v`
supports both. Without `Direction`, the direct form uses the only closed axis
or U when both close. It starts each relocated domain at the requested
parameter. Rational and periodic tensor structure, object identity,
attributes, and groups are preserved. Boundaries whose stored homogeneous
seam controls do not close are rejected.

`Reparameterize start end` changes one selected curve's domain, while four
values set the U and V domains of one selected untrimmed NURBS surface. Commas
between values are also accepted. `Reparameterize Automatic` uses curve length
or OpenNURBS' longest-control-polygon surface size. Curves retain their native
representations. Polycurves redistribute outer intervals by segment length, matching
Rhino's Domain setter; other curves use affine parameter maps. Geometry, orientation,
periodicity, object identity, attributes, groups, selection, and undo are
preserved.
