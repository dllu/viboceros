# NURBS structure and parameterization

[Command reference](README.md) · [Project overview](../../README.md)

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

`InsertControlPoint point Direction=U|V|Both Midpoint=Yes|No` requires exactly
one selected curve or untrimmed NURBS surface. The model-space point is
projected to the object, then a unit-weight control is inserted between the
bracketing control-point Greville parameters, matching Rhino and generally
changing shape. `Midpoint=Yes` snaps the new control and knot to the middle of
that interval; omit the point to pick it in a viewport. On surfaces, Direction
names the row orientation as it does in Rhino (a U row adds a V-axis control);
U is the default. Rational and periodic inputs, object identity, attributes,
selection, and undo are preserved.

`InsertKnot` requires exactly one selected curve or untrimmed NURBS surface and
refines it in place without changing its parameterization or shape. Curves take
one parameter; surfaces take `u,v`, with a scalar accepted when `Direction=U`
or `V`. `Multiplicity` is the target knot multiplicity and may range from one
through the relevant degree. `Symmetrical=Yes` also inserts the parameter
mirrored across the active domain. Rational refinement and periodic repair
follow Rhino/OpenNURBS behavior; identity, attributes, selection, and undo are
preserved.

`RemoveKnot parameter|u,v Direction=U|V` requires exactly one selected curve
or untrimmed NURBS surface and removes one knot in place. On curves, the
parameter identifies a curve point and the knot with the nearest model-space
point is selected, matching Rhino. On surfaces, U is the default and the knot
value nearest the chosen coordinate is removed consistently across the whole
control net. Remaining homogeneous controls are interpolated at Greville
abscissae; rational weights, object identity, attributes, selection, and undo
are preserved. Periodic directions are rejected, while non-clamped directions
are first clamped without changing their active domain.

`RemoveControlPoint index Direction=U|V` requires exactly one selected curve
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

## Exact conversion

`ToNURBS` exactly converts selected lines, circles, arcs, ellipses, and
polylines. It retains inputs and creates unselected copies in the same groups
by default; use `DeleteInputObjects=Yes` to preserve object identities and
replace the inputs in place. Line, polyline, circle, arc, and ellipse parameter
domains follow Rhino's chord-length, arc-length, and angular conventions.

## Span decomposition

`ConvertToSingleSpans` decomposes selected untrimmed NURBS surfaces at their
exact knot spans in `Direction=U`, `V`, or `Both`. Rational weights, source
parameter values, attributes, and group membership are preserved. Inputs are
retained by default; `DeleteInput=Yes` replaces them in one undoable edit, and
surfaces already single-span in the requested direction are left untouched.

`ConvertToBeziers` decomposes selected NURBS curves and untrimmed NURBS surfaces
exactly at every nonempty knot span. It preserves rational weights, source
parameters, attributes, and group membership. Inputs are retained by default;
`DeleteInput=Yes` replaces them with fresh pieces in one undoable edit. A
single-span input still produces a fresh Bezier object.

## Seams and parameter domains

`SrfSeam point Direction=U|V|Both` performs the corresponding exact edit on
one selected untrimmed NURBS surface; omit the point for a one-click viewport
pick. `Parameter=value` targets one axis directly, while `Parameter=u,v` also
supports `Direction=Both`. Without `Direction`, the only closed axis is chosen,
or U is used when both axes close. The implementation follows OpenNURBS by
flattening the stored homogeneous control net, so rational and periodic tensor
structure, parameter-span lengths, surface orientation, identity, attributes,
groups, selection, and undo are preserved. A projectively coincident boundary
whose stored homogeneous seam controls do not close is rejected as Rhino does.

`Reparameterize start end` changes one selected curve's domain, while four
values set the U and V domains of one selected untrimmed NURBS surface. Commas
between values are also accepted. `Reparameterize Automatic` uses curve length
or OpenNURBS' longest-control-polygon surface size. Curves retain their native
representations. Polycurves redistribute outer intervals by segment length, matching
Rhino's Domain setter; other curves use affine parameter maps. Geometry, orientation,
periodicity, object identity, attributes, groups, selection, and undo are
preserved.
