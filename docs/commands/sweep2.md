# Sweep2

`Sweep2 [Rail1Name=name Rail2Name=name] [Rebuild=point-count|Refit=tolerance]`
`[MaintainHeight=Yes|No] [RefitRails=Yes|No] [StartPoint=x,y,z] [EndPoint=x,y,z]`

Pick the first rail, the second rail, and then the cross-section curves.
As soon as the inputs define a valid surface, Sweep2 draws a wire
preview. Enter opens the options review. If all inputs are preselected, the
bare `Sweep2` command opens review directly. Enter commits the preview and
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
from their endpoints, the terminal profile determines which side to keep.
Once a profile is available, the preview follows the snapped cursor. Picking
the last point opens review. A point end counts as a section, and at least
one real profile is required. Scripts can use `StartPoint=` and `EndPoint=`.

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

Choose `RefitRails=Yes` to fit both rails in a shared cubic basis. Between
profile contacts the sweep pairs the rails by relative distance traveled. With
multiple profiles, the first and last contacts bound the retained interval of
each rail. Sweep2 fits the rails automatically for shortened sweeps and when
interior contacts have different normalized parameters. The fitted boundaries
are checked against the retained rail intervals at the document tolerance.

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

The solver requires open rails and open polynomial sections. Rail refitting
accepts rational rails, and Rebuild can prepare polynomial sections from
rational profiles. Section endpoints must contact both rails. With multiple
sections, contacts must progress in order on both rails. The unrefitted native
skin requires more sections than the maximum rail degree. Closed sweeps,
slash guides and surface-edge continuity remain unsupported. Failures leave
the document and history unchanged.

Construction uses either usd-brep's rail-constrained skin or a transported
profile in a shared rail basis. An independent continuous deviation
certificate checks each retained rail interval and cross section against the
surface in world coordinates. An inconclusive certificate rejects the result.
Kinks in an accepted tensor are split into connected B-rep faces.

Limits are 32 sections, degree 16, 128 controls per input curve and 1,024 total
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
