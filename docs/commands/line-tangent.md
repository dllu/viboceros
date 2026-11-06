# Line Tangent

Start `Line Tangent`, or choose `Tangent` at Line's first prompt. Pick a curve,
then locate the endpoint. The contact slides to a tangent reaching that point.
An open curve can supply an [endpoint tangent](line-tangent-boundaries.md)
when no exact tangent reaches the requested point. Other unusable endpoints
keep the getter open for another attempt.
Picking different parameter branches of a closed curve can choose different
contacts; the seam affects this choice.

| Option | Behavior |
| --- | --- |
| `FromFirstPoint` | Fix the picked contact and project the endpoint onto its tangent line. Enter at the sliding endpoint prompt also fixes it. |
| `PointOnCurve` | Select a curve and locate a start on it; its tangent is fixed immediately. |
| `Point` | Override the forced curve snap for the next location. A free start can snap to a tangent on another curve, or to its near point if no tangent reaches it. |
| `2Curves` | Pick two curves near their desired contacts to construct a bounded common tangent. No usable tangent finishes without an object. BothSides is ignored. |
| `BothSides` | Mirror the endpoint around the contact. |

Signed numbers lock the length before the direction point. A fixed tangent with
BothSides finishes from the number alone; its sign chooses endpoint order.
Planar projects a sliding line parallel to the active CPlane through its contact.
A fixed tangent takes precedence over Planar. Escape cancels; source replacement,
deletion, hiding, locking or tolerance changes require a fresh pick.

Scripts bind a curve or Brep edge followed by its contact pick:

```text
Line Tangent Curve=1 5,0,0 8,6,0
Line Tangent Curve=1 5,0,0 FromFirstPoint 8,6,2
Line Tangent PointOnCurve Curve=1 5,0,0 3,4,0 8,6,2
Line Tangent 2Curves Curve=1 0,5,0 Curve=2 12,2,0
```

Use current object IDs. `Edge=id,index` selects an edge; `Planar=Yes|No` supplies
script drafting context. Interactive getters use the Planar control.
`ReadCommandFile` accepts separate getter tokens. PointOnCurve and 2Curves clear
selection; ordinary picks preserve it. Completion creates one line on the current
layer with one Undo/Redo entry. Source geometry and attributes stay intact.

The [validation record](../line-tangent-validation.json) retains 73 native
observations: 58 complete inputs and 15 incomplete driver recipes. Each build
replays 55 matching inputs, including 50 constructions, five no-output controls
and 100 3DM round trips. Three repeated captures of one open-arc input differ:
Rhino uses an endpoint tangent while Viboceros finds an interior tangent. These
remain explicit known differences, outside the passing comparison counts.

Matching coefficients, domains and 33 locally evaluated samples use a `1e-6`
gate. Independent native PointAt samples, arbitrary singular curves, all common
tangents, continuous distance and comparative speed remain unqualified.
[Mouse-mode checks](line-tangent-mouse.md) cover CPlane rejection, Planar,
boundary projection and the remaining perspective pick calibration difference. Run `python3 -m tools.rhino_oracle.check_line_tangent` without opening
a desktop window. See the public [Rhino Line reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/line.htm).
