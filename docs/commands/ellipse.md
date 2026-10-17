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

`FromFoci` offers `MarkFoci=Yes|No`; bare `MarkFoci` toggles it. The two markers
are added before the curve, in second-focus/first-focus order. This setting is
remembered after successful commands, independently of document Undo/Redo and
within one command registry. Other construction modes create no focus markers.
Cancelled pending options do not save preferences. Each invocation starts in
Default construction mode.

Examples of complete scripts:

```text
Ellipse 0,0,0 6,0,0 2,3,0
Ellipse Vertical 0,0,0 6,0,0 0,0,3
Ellipse Diameter -6,0,0 12 2,0,0 6 0,4,2
Ellipse Corner -6,-3,0 6,3,0
Ellipse 1,2,3 Corner 7,10,7
Ellipse FromFoci 0,0,0 8,0,0 4,3,0 MarkFoci=Yes
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

Five clean native failures and three cancellations are separate diagnostics.
Six additional cases have incomplete input receipts, uncontrolled cursor-based
Enter input, negative numeric behavior, or an implicit preference dependency;
they do not qualify successful geometry replay. Local locked-preview Enter is
covered by frontend tests. Native failure termination and preference updates on
cancellation are not claimed: local invalid picks allow retries and pending
options save only after success.

`AroundCurve` and deformable output remain unimplemented. Negative numeric
sizes are rejected. Exhaustive physical mouse/snap, extreme-range and performance
parity remain unqualified.

Reference: McNeel's public
[Ellipse command documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/ellipse.htm).
