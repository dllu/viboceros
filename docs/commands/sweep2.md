# Sweep2

`Sweep2 [Rail1Name=name Rail2Name=name] [Rebuild=point-count|Refit=tolerance]`
`[StartPoint=x,y,z] [EndPoint=x,y,z]`

Pick the first rail, the second rail, and then the cross-section curves.
As soon as the inputs define a valid surface, Sweep2 draws a wire
preview. Enter opens the options review. If all inputs are preselected, the
bare `Sweep2` command opens review directly. Enter commits the preview and
Escape cancels. Local `Undo` steps back through settings, review, and picks.
Finishing creates one B-rep in one model undo step.

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

The solver requires open polynomial rails and open polynomial sections. Rebuild
can also prepare polynomial sections from rational profile curves. Section
endpoints must contact both rails. Without point ends, the first and last
sections must contact the ends of the complete rails. Interior sections must
meet both rails at matching normalized rail parameters. The total section
count must exceed the maximum rail degree. Closed sweeps, rational rails,
single profiles and partial rails without point ends, independent station
parameterizations, height options, rail refitting, slash guides and
surface-edge continuity remain unsupported. Failures leave the document
and history unchanged.

Construction delegates to usd-brep's `SmBSplineSurface::CreateSkinnedSurface`
with both rail constraints and interpolated cross sections. Native curve
compatibility works on private copies. An independent continuous deviation
certificate checks each retained rail interval and cross section against the
resulting surface in the original world coordinates before it reaches the
document. An inconclusive certificate rejects the result. This certificate
does not establish surface injectivity or arbitrary interior-shape parity with
Rhino. Kinks in an accepted tensor are split into connected B-rep faces.

Limits are 32 sections, degree 16, 128 controls per input curve and 1,024 total
input controls. A conservative compatibility control-net limit and the shared
native transfer limits bound output allocation; command output is limited to
4,096 faces.

Eight fresh licensed Rhino fixtures compare rectangles, tapered widths,
constant/changing arches, bent rails, reversed directions, independent domains
and a remote origin. All 160 world-space queries agree within `1e-6` model
units; areas agree within `1e-5` square model units. The largest point difference
is `9.31e-7` at the remote origin. These are finite fixture comparisons.

Rhino 8.32 command captures match the complete straight-rail arch image
with a start or end point, including at an interior rail crossing. With
multiple shaped profiles, Rhino's blending between sections can differ from
the constrained skin used here.

Validation and raw reference observations are retained in the
[qualification record](../sweep2-qualification.json). The public SDK reference
arranges its own private curves using Rhino endpoint/closest-point queries,
then calls the two-rail `Brep.CreateFromSweep` overload. It does not reproduce
Rhino's native command getter, options, document metadata or history.

References: [Rhino Sweep2](https://docs.mcneel.com/rhino/8/help/en-us/commands/sweep2.htm),
[public sweep API](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.brep/createfromsweep).
