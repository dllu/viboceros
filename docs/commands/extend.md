# Extend and ExtendDynamic

[Curve commands](curves-index.md) · [Rhino Extend](https://docs.mcneel.com/rhino/8/help/en-us/commands/extend.htm)

Run `Extend` and select one or more curve, surface, or B-rep boundaries. Press
Enter, then pick a curve or surface edge near the end to extend. Pick the end
again to continue to the next intersection. Enter a length at the first prompt
to apply a fixed distance to each subsequent pick. A negative length shortens
the picked end. Preselected objects can serve as boundaries.

Press Enter at the empty boundary prompt, or start `ExtendDynamic`, to pick a
curve end and draft the extension with the mouse. Click the preview or type a
point or signed length. Straight extensions project the input onto the tangent.
`ExtendDynamic` offers `ToBoundary` to switch to boundary picking.

`Type=Natural|Arc|Line|Smooth` and `Join=Merge|Yes|No` remain editable during the
command. With `Type=Arc`, `ToPoint` constructs a tangent arc and `Center` asks
for a center followed by an endpoint or length. With `Type=Smooth`, `ToPoint`
builds a curvature-continuous span ending at the supplied point. With a source
selected by `SelID`, type `Start` or `End` to identify its end explicitly.
`Undo` inside the command reverses its latest accepted extension. Enter or Esc
ends the command, with accepted steps grouped into one model Undo entry.

Point-defined smooth Merge reduces the seam knots on polynomial inputs.
Rational inputs retain their seam. Point-defined arcs merge on the same
supporting circle when the result fits in one arc. Extending a surface edge
creates a curve and retains its parent surface.

## Scripted forms

`Extend Length=value Side=Start|End|Both` extends one selected open curve;
`Both` adds the full requested model-space length independently at each end.
`Type=Natural` selects the source end's natural line, same-radius arc, or smooth
continuation, while `Type=Smooth` explicitly requests curvature-continuous
free-form extrapolation. `Type=Arc` follows the endpoint's exact osculating
circle and falls back to a degree-matched tangent line at zero curvature;
requests beyond one revolution use Rhino's full-circle cap while retaining the
requested parameter-domain interval. `Type=Line` uses Rhino's straight tangent extension.
With `Join=Merge`, line and polyline ends merge into their terminal span,
higher-degree straight curves collapse to one line, and other curves retain
Rhino's exact degree-matched span, knot, and rational-weight rules. A merged
same-radius arc is rebuilt as one canonical rational arc. `Join=Yes` retains an
explicit segment boundary, including Rhino's unit-span polyline
parameterization and exact full-multiplicity free-form seam. `Join=No` leaves
the source untouched and creates one or two native line, arc, or smooth
extension curves with copied attributes but without copying source group
membership. Rhino's post-command deselection and undo behavior are retained.

`Extend point Type=Natural|Arc|Line|Smooth Join=Merge|Yes|No` extends one of
the selected open curves to the nearest intersection with the other selected
curve, surface, or trimmed B-rep boundaries. The point chooses the closest
source endpoint. Exact span subdivision handles
transverse and tangent curve-surface hits, while finite coplanar overlaps stop
at their entry edge and B-rep trim holes are excluded. Intersections already
on the source are ignored, multiple boundaries resolve to the first forward
hit, and the source identity, attributes, groups, undo state, and Rhino's
separate-output behavior are preserved.

`Extend Domain=start,end` exposes exact analytic NURBS-domain extension.
Analytic curves and polylines are promoted to exact NURBS form as needed while
identity is preserved. The permanent length-command fixture covers all join
modes for explicit Smooth; its `2e-8` threshold contains Rhino's approximately
`1.2e-8` cubic length-solver variation while Viboceros resolves that case to
machine precision. The boundary-command fixture covers every continuation
style, all join modes, both-end extension, nearest-boundary selection,
standalone surfaces, solid and trimmed B-reps, curved and tangent surface hits,
coplanar overlap, and trim holes to `1e-10`. Arc cases compare a canonical
knot parameterization because Rhino leaks its temporary boundary-search extent
into geometrically equivalent segment domains. Arc geometry and commands
otherwise agree to floating-point roundoff except for the same pre-existing
Smooth length-solver variation.
