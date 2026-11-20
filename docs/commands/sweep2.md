# Sweep2

`Sweep2 [Rail1Name=name Rail2Name=name] [Natural|Automatic] [Rebuild=point-count|Refit=tolerance]`
`[MaintainHeight=Yes|No] [RefitRails=Yes|No] [Closed=Yes|No] [StartPoint=x,y,z] [EndPoint=x,y,z]`
`[Slash=x,y,z/x,y,z ...]`

Pick the first rail, the second rail, and then the cross-section curves.
As soon as the inputs define a valid surface, Sweep2 draws a wire
preview. Enter advances to the seam editor for closed profiles or the options
review for open profiles. With preselected inputs, `Sweep2` opens that stage
directly. Enter in review commits the preview and
Escape cancels. Local `Undo` steps back through settings, review, and picks.
Finishing creates one B-rep in one model undo step.

Choose `ChainEdges` while selecting a rail to build it from connected curves
or B-rep edges. The picker highlights the chain as it grows. Use `AutoChain`,
`ChainContinuity`, `Direction`, `GapTolerance`, or `AngleTolerance` to control
growth. With `AutoChain=No`, pick segments individually or double-click to
grow the chain. `Next`, `All`, and `Undo` edit the chain before Enter accepts
it. Accepting the first chain opens the second rail picker. The sweep uses
private joined rails and leaves their pieces intact.

With converging rails, choose `Point` before the first profile or after the
last profile. `StartPoint` and `EndPoint` name either end directly. Click or
type a point within model tolerance of both rails. If the rails cross away
from their endpoints, pick profiles on one side of the crossing. The sweep
can end at a profile within that retained rail interval.
Once a profile is available, the preview follows the snapped cursor. Picking
the last point advances the command. A point end counts as a section, and at least
one real profile is required. Scripts can use `StartPoint=` and `EndPoint=`.

For closed profiles, drag the seam markers to change how the curves line up.
The surface preview follows the drag. `Flip` reverses the profile at the
selected marker, `Automatic` aligns the seams and directions, and `Natural`
restores their original positions and directions. `SnapToKnots=Yes` limits
movement to profile knots. Choose `SnapToKnots=No` to move between knots.
You can also use `SelID` to select a profile, then type a point or
`Parameter=value`. Press Enter to review the sweep.

Use `Simplify` to choose `None`, `Rebuild`, or `Refit`. Rebuild accepts 2 to
128 control points per section, and Refit accepts a positive tolerance in
model units. Sweep2 remembers both values while you switch modes. The wire
preview updates to reflect each accepted value. Choose `Simplify=None` to
keep the original section curves. The source objects stay intact in every mode.

One profile is enough to preview and create a sweep. If it meets both rails
at interior stations, the sweep starts there and follows the first rail's
direction. Choose `MaintainHeight=Yes` in review to scale the profile's
height independently of rail width. The height is measured across the rail
pair, perpendicular to the width and average sweep direction. Watch the
preview while changing the option, or use local `Undo` to restore it.

With two closed rails, the first profile places the seam. A single profile
sweeps the full loop. With multiple profiles, pick them in order around the
rails. `Closed=No` ends at the last profile. Choose `Closed=Yes` in review
to continue back to the first profile and join the seam. The preview updates
as you switch between the two options. With two profiles, the first rail's
direction determines which arc to use for `Closed=No`.

Choose `RefitRails=Yes` to fit both rails in a shared cubic basis. Between
profile contacts the sweep pairs the rails by relative distance traveled. With
multiple profiles, the first and last contacts bound the retained interval of
each rail. Sweep2 fits the rails automatically for shortened sweeps and when
interior contacts have different normalized parameters. The fitted boundaries
are checked against the retained rail intervals at the document tolerance.

In review, choose `AddSlash` to pair points between the two retained rails.
Pick or type near either rail, then pick or type the point for the opposite
rail. The points are projected onto the rails. The guide and the sweep
preview follow the second point. Add more pairs or use local `Undo`, then
press Enter to return to review. Escape discards an
unfinished pair and returns to review. The pairs must follow the same order
on both rails, including the profile contacts. Scripts can repeat
`Slash=x,y,z/x,y,z`, with one point per rail in selection order.

For example, in a new document:

```text
Line 0,0,0 0,0,5
Line 2,0,0 2,0,5
Line 0,0,0 2,0,0
Line 0,0,5 2,0,5
SelAll
_-Sweep2
```

For named rails, supply both names and select just the cross sections. An
explicit option or `_-Sweep2` runs directly with complete preselection. Rail
names must be unique, and the rails must be visible and unlocked. Names are
case-insensitive and currently cannot contain whitespace in this command.
Sections may be selected in either order and either direction. The second rail
is reversed automatically when necessary to align the section contacts.

The solver accepts open or closed rail pairs and sets of open or closed
profiles. Open profiles are polynomial, and their endpoints must contact both
rails. Closed profiles may be rational and use the rails to guide their frame
and scale. All profiles in one sweep must have the same closure. With
multiple sections, contacts must progress in order on both rails. Exact
sweeps can preserve a common rational rail basis. Rail refitting accepts
rational rails, and Rebuild can prepare polynomial sections from rational
profiles. The unrefitted native skin for open rails and open profiles requires
more sections than the maximum rail degree. Failures leave the document and
history unchanged.

Construction uses either usd-brep's rail-constrained skin or a transported
profile in a shared rail basis. An independent continuous deviation
certificate checks the cross sections and, with open profiles, both retained
rail boundaries against the surface in world coordinates. An inconclusive
certificate rejects the result.
Kinks in an accepted tensor are split into connected B-rep faces.
Exact rational sweeps with closed rails and open profiles also split at
full-multiplicity rail knots.

Limits are 32 sections, 32 slash pairs, degree 16, 128 controls per input curve and 1,024 total
input controls. A conservative compatibility control-net limit and the shared
native transfer limits bound output allocation; command output is limited to
4,096 faces.

Eight fresh licensed Rhino fixtures compare rectangles, tapered widths,
constant/changing arches, bent rails, reversed directions, independent domains
and a remote origin. All 160 world-space queries agree within `1e-6` model
units; areas agree within `1e-5` square model units. The largest point difference
is `9.31e-7` at the remote origin. These are finite fixture comparisons.

Rhino 8.32 `ChainEdges` captures give one face for collinear pieces and two
joined faces for kinked pieces. The chained Sweep2 fixtures match the captured
face and edge counts and areas within `1e-8` square model units.

Closed-rail command captures cover nested squares and concentric circles,
including partial arcs, a single profile, reversed rail direction, and curved
profiles. Exact fixtures match captured face and edge counts and areas within
`1e-5` square model units.

Closed-profile captures cover circular tubes, tapers, asymmetric guides,
square profiles and point ends. Smooth profiles form a joined seam in the
transverse direction. Closing the sweep along the rails can join the other
direction as well.

Rhino 8.32 command captures match the complete straight-rail arch image
with a start or end point, including at an interior rail crossing. With
multiple shaped profiles, Rhino's blending between sections can differ from
the constrained skin used here.

Tapered straight-rail captures cover one, two, and three profiles with
`MaintainHeight`, including a single interior profile. On curved or skew
rails the local transport can differ from Rhino's interior shape.

The `RefitRails` captures cover tapered straight rails, an interior profile
paired across different stations, and curved rails. Partial-rail captures
also cover independent terminal stations and reverse pick order. Curved-rail
fitting can choose different knots from Rhino.

Validation and raw reference observations are retained in the
[qualification record](../sweep2-qualification.json). The public SDK reference
arranges its own private curves using Rhino endpoint/closest-point queries,
then calls the two-rail `Brep.CreateFromSweep` overload. It does not reproduce
Rhino's native command getter, options, document metadata or history.

References: [Rhino Sweep2](https://docs.mcneel.com/rhino/8/help/en-us/commands/sweep2.htm),
[public sweep API](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.brep/createfromsweep).
