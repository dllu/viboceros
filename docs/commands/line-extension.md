# Line Extension

Choose `Extension` at Line's first prompt, or start `Line Extension`. Pick a
curve near the end to extend, then pick or type an endpoint. Viboceros adds a
separate line along that endpoint's tangent and leaves the reference unchanged.
The final point projects onto the whole tangent line, so it can also lie back
toward the source. `BothSides` mirrors the extension around its starting point.

The picked station chooses the nearer endpoint by arc length along the curve.
Physical distance to the endpoints and the midpoint of the parameter domain
both select the wrong end on some curves. Closed curves are accepted; their
seam can have different tangents on its two sides.

A positive number at the endpoint prompt locks the length. A negative number
sets a length increment. Then pick or type a point to choose the direction, or press Enter to accept
the remembered cursor direction. Moving the mouse updates that direction.
Line, arc, circle, polyline, polycurve and weighted NURBS references share the same finite tangent
constraint and native parameter-domain policy as other Line modes.

Complete commands can bind an existing object with
`Line Extension Curve=id near-point endpoint`, or a B-rep/surface edge with
`Line Extension Edge=id,index near-point endpoint`. Supply the near-point and
endpoint as world `x,y,z`; a length token can precede the endpoint. The interactive
prompt and `ReadCommandFile` also accept the binding and near-point as separate
tokens. Object numbers refer to the current document. In a complete command,
a length followed by `Enter` uses the near-point as the remembered cursor.
Interactive lengths also accept the drafting calculator and model-unit suffixes.

Choosing Extension clears existing selection. The reference is highlighted
while the getter is active and released on completion or cancellation. Picks,
length locks and previews stay outside document geometry. Completion adds one
Undo/Redo entry; cancellation preserves pending Redo. Replacing a reference,
changing tolerance or making it unavailable requires picking again. The parent
object's identity, geometry storage, attributes, group membership and topology
are preserved.

The [qualification record](../line-reference-validation.json) retains 36 complete
native workflows with 34 acknowledged physical curve picks. Each build checks
30 constructions and six cancellation controls, plus 60 valid 3DM round trips
across direct commands and getters. The `1e-6` gate covers coefficients, weights,
knots, domains and 33 locally evaluated samples. Folded and closed curves,
spatial tangents, coordinates near 10¹², reversed endpoints, BothSides, fixed and
incremental lengths, coarse document tolerance and pending Redo are covered.
Windowless tests also exercise viewport clicks, preview/commit agreement,
command-file input, source replacement, transparent CPlane input, numeric
calculator/unit routing and B-rep edge bindings.

Native component-pick geometry, exact arc-length midpoint endpoint ties, event-time source
highlighting, independent native PointAt samples, continuous distance and
comparative performance remain unqualified. Run the checks with
`python3 -m tools.rhino_oracle.check_line_reference`.

See the public [Rhino Line reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/line.htm) for its command options.
