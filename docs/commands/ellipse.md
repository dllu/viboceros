# Ellipse

Start `Ellipse` (alias `Ell`) and pick a center, the first axis end, and the
second axis end. The first axis fixes the seam and its full 3D length. The
second pick determines the plane; its component perpendicular to the first
axis determines the other radius. For example, center `0,0,0`, first axis
`6,0,0`, and second pick `2,3,0` produce radii 6 and 3.

The same edit-free input constructs previews and final geometry. Picks use
shared object snapping, point filters, units, and world/CPlane/relative input.
Invalid picks leave the current getter available for correction.

| Construction | Input and behavior |
| --- | --- |
| Default | Center and two axis ends; axes may be spatial. |
| Vertical | Center and two axis ends; the ellipse plane contains the first axis and CPlane normal. If they are parallel, the final pick determines the plane. |
| Diameter | Two ends of the first diameter, then a second-axis end measured from their midpoint. `Vertical` is also available before the first end. |
| Corner | Two bounding-box corners, projected into the CPlane through the first corner. The seam follows positive CPlane X. |
| Corner after center | Choose `Corner` after the default center pick to retain that center and pick a bounding-box corner. |
| FromFoci | Two distinct foci and a point on the ellipse. A collinear point beyond the foci uses the CPlane to resolve the plane. A point between them cannot define a finite ellipse. |
| AroundCurve | Select a curve or surface/B-rep edge, pick its center on the guide, then two axis ends in the plane perpendicular to the guide tangent. |

At either axis getter, a positive length locks the size; pick a direction to
continue. A replacement number changes the lock. Default and Vertical numbers
are radii. Diameter numbers are full diameters for **both** axes; a picked
second-axis end still specifies a radius. Corner accepts a locked diagonal
length before its final pick; it constrains the full 3D distance before CPlane
projection. Numeric input accepts document length units.

Enter with a locked length accepts the current viewport preview. Move the
pointer over the viewport first; without a current hover, the getter remains
available. Enter without a lock cancels. Escape and `Cancel` always cancel.
Relative coordinates use the last accepted point and active CPlane vector axes.
For AroundCurve, the accepted center is the closest point on the guide and axis
picks are projected into its normal plane before becoming the relative base.

AroundCurve accepts one preselected curve, a viewport curve/edge pick, or
`Curve=<object-id>` / `Edge=<object-id>,<edge-index>`. Both axis getters support
numeric radius locks and live previews. The source remains untouched. A guide
that changes, disappears, or becomes hidden/locked must be selected again, as
must a guide captured before a document tolerance change, even after its center
has been picked. This freshness check is local robustness behavior.
Cancellation creates no helper objects.
Postselected guides are released when the command returns; preselected guides
remain selected. Each completed ellipse forms one Undo/Redo operation.

`FromFoci` offers `MarkFoci=Yes|No`; bare `MarkFoci` toggles it. The two markers
are added before the curve, in second-focus/first-focus order. This setting is
remembered after successful commands, independently of document Undo/Redo and
within one command registry. Other construction modes create no focus markers.
Cancelled pending options do not save preferences. Each invocation starts in
Default construction mode.

Choose `Deformable` (or `Deformable=Yes`) before the first point to create a
non-rational periodic cubic approximation. `PointCount=8`, or `PointCount`
followed by an integer, sets its unique control count; three repeated seam
controls are stored in addition. The default count is 12. Supported counts are
4 through 1000. Enter at the count prompt accepts its current value; Escape or
`Cancel` cancels. Counts below four leave that prompt available for correction.
Point counts are dimensionless and accept no length-unit suffixes.

Deformable output works with every construction mode, numeric axis locks and
focus markers. Previews draw the same approximation used by the final command.
The shape interpolates equal-angle ellipse stations using the shared periodic
cubic solver; its parameter domain is zero through the semi-axis diagonal
length. A successful command remembers the point count across Undo/Redo.
Each new invocation starts with exact output and must enable Deformable again.
As with MarkFoci, local cancelled option edits do not save preferences.

Rhino 8.32's native Ellipse getter offers `PointCount` and produces cubics; it
rejects `Degree`, despite the broader description in the public help. Captured
degree attempts remain diagnostics. Deformable cannot be disabled after being
enabled in the same invocation; cancel and restart for exact output.

Examples of complete scripts:

```text
Ellipse 0,0,0 6,0,0 2,3,0
Ellipse Vertical 0,0,0 6,0,0 0,0,3
Ellipse Diameter -6,0,0 12 2,0,0 6 0,4,2
Ellipse Corner -6,-3,0 6,3,0
Ellipse 1,2,3 Corner 7,10,7
Ellipse FromFoci 0,0,0 8,0,0 4,3,0 MarkFoci=Yes
Ellipse Deformable PointCount=8 0,0,0 6,0,0 0,3,0
```

The curve and optional markers use current object defaults and form one undo
operation. Previews and retries create no model objects. Complete invocations
and frontend input share construction and history code. The geometry API's
`Ellipse3::try_from_three_points` retains RhinoCommon's full-vector second-radius
convention; the command uses the measured perpendicular-radius rule instead.

## Verification and limits

[Validation](../ellipse-validation.json) compares 55 successful Rhino 8.32
getter workflows through the command and frontend paths at absolute `1e-6`.
Checks include all NURBS controls, weights, knots, domains, lengths, focus-marker
ordering, selection, and actual Undo/Redo. Inputs cover skew and spatial axes,
CPlanes, both Corner forms, diameter sizing, numeric locks and replacements,
relative coordinates, collinear focal fallbacks and remembered focus markers.
Kernel tests also preserve a thin ellipse whose major radius rounds to its
focal distance; its minor radius is computed without subtracting squared lengths.

[AroundCurve validation](../ellipse-around-validation.json) adds 27 successful
native workflows at `1e-6`, including analytic/NURBS/polyline guides, endpoints,
off-guide centers, projected and locked axes, rotated relative input, and four
owned surface/B-rep edge clicks. Command and frontend checks compare complete
NURBS geometry, lengths, unchanged sources, selection and actual Undo/Redo.
Returned-state selection uses the native `after_script` snapshot: Rhino's
earlier EndCommand snapshot temporarily retains a postselected guide.
Five clean cancellations and one failure are separate controls; a timed-out
zero-axis retry remains an excluded diagnostic. Local tests additionally cover
source freshness, point filters, parameter picks and cancellation cleanup.

[Deformable validation](../ellipse-deform-validation.json) compares 33 successful
native workflows at `1e-6` through command and frontend on both backends: 32
periodic cubic outputs and one restart with exact output. Coverage includes
counts 4 through 64, all construction modes, a B-rep edge route,
remembered counts, the integer getter, Enter defaults and a corrected minimum
count. Two clean cancellations create no output. Nine separate diagnostics
retain six unknown-option attempts, two superseded prompt-adapter timeouts,
and one implicit preference dependency. A local 1000-point test checks control
count and station interpolation; native high-count and cancellation-preference
parity remain unqualified.

The earlier mode validation retains five clean native failures and three
cancellations as separate diagnostics.
Six additional cases have incomplete input receipts, uncontrolled cursor-based
Enter input, negative numeric behavior, or an implicit preference dependency;
they do not qualify successful geometry replay. Local locked-preview Enter is
covered by frontend tests. Native failure termination and preference updates on
cancellation are not claimed: local invalid picks allow retries and pending
options save only after success.

Negative numeric sizes are rejected. Exhaustive physical mouse/snap, extreme-range and performance
parity remain unqualified.

Reference: McNeel's public
[Ellipse command documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/ellipse.htm).
