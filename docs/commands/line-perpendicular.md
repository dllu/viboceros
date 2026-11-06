# Line Perpendicular

Start `Line Perpendicular`, or choose `Perpendicular` at Line's first prompt.
Pick a curve, then pick or type an endpoint. The contact slides along the curve
to keep the line perpendicular. Picking the opposite side of a circle selects
its farther normal. At an open curve boundary, the contact stops and the
endpoint projects into that contact's normal plane.

| Option | Behavior |
| --- | --- |
| `FromFirstPoint` | Fix the picked contact and constrain the endpoint to its normal plane. Enter at a sliding endpoint prompt also chooses this mode. |
| `PointOnCurve` | Select a curve, then locate the start on it. This fixes the contact immediately. |
| `Point` | Override the forced curve snap for the next location. A free start can be followed by a typed endpoint or a perpendicular curve pick. If that curve has no usable perpendicular, its near point can be accepted. |
| `2Curves` | Pick two curves near the desired contacts. Construct their bounded common perpendicular, or finish without an object if no usable segment exists. This mode ignores BothSides. |
| `BothSides` | Choose before the first point to mirror the endpoint around the contact. |

At an endpoint prompt, a signed number locks the length; a following point
chooses direction. Negative lengths reverse direction. A sliding contact moves
before applying that length. In a fixed-contact getter, Enter cancels.

Planar mode keeps a sliding line parallel to the active CPlane, including for
explicit 3D coordinates. A fixed contact uses its normal plane instead. Curve
picks and endpoint previews use the original geometry. Reference replacement,
deletion, hiding, locking or tolerance changes require a fresh pick.

Scripts bind `Curve=id` or `Edge=id,index`, followed by a contact pick:

```text
Line Perpendicular Curve=1 5,0,0 7,4,3
Line Perpendicular PointOnCurve Curve=1 5,0,0 3,0,0 7,4,3
Line Perpendicular Point 8,6,2 Curve=2 5,0,0
Line Perpendicular 2Curves Curve=1 5,0,0 Curve=3 2,6,5
```

Use current document object IDs. `Planar=Yes|No` supplies drafting context to
complete script commands; interactive getters use the Planar control.
`ReadCommandFile` accepts the same getter tokens separately. Ordinary snap picks
preserve existing selection; PointOnCurve and 2Curves clear it. Sources retain
their identity, geometry storage, attributes, groups and topology. Completion
creates one line and one Undo/Redo entry on the current layer.

The [qualification record](../line-perpendicular-validation.json) retains 64
native captures: 56 complete workflows and eight driver diagnostics. Each build
checks 48 constructions, eight no-output controls and 96 valid 3DM round trips.
Coefficients, domains and 33 locally evaluated samples use a `1e-6` gate.
Physical cursor inputs are calibrated from public SDK curve/viewport queries,
independently of native output geometry. Windowless egui tests also cover source,
base and endpoint picking, preview/commit equality and relative coordinates.

Arbitrary singular curves, global completeness of common-perpendicular solutions,
native component-edge picking, independent native PointAt samples, continuous
distance and comparative speed remain unqualified. Line Tangent is still
unsupported. Run `python3 -m tools.rhino_oracle.check_line_perpendicular`.
See the public [Rhino Line reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/line.htm).
