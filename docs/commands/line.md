# Line construction

`Line` (`L`) creates one line object. Pick or type its start and end, or use a
complete command such as `Line 1,2,3 5,5,7`. Choose a construction mode before
the first point:

| Mode | Inputs and behavior |
| --- | --- |
| `BothSides` | Treat the start as the midpoint and mirror the final endpoint. Combine with another mode, or choose it after the start. |
| `Vertical` | Constrain the endpoint along the initial construction-plane normal through the start. |
| `FourPoint` | Supply two reference points, then a start and endpoint. Both latter points project onto the reference line; the final getter follows its direction through the projected start. |
| `Bisector` | Supply a pivot, one point on each ray, then an endpoint. The endpoint projects onto the internal spatial angle bisector. Exactly opposite rays cancel. |
| `Angled` | Supply two base-line points, an angle in degrees, then an endpoint. Rotate the base line around the initial construction-plane normal and project onto the rotated line. Spatial base lines retain their normal component. |
| [`Perpendicular`](line-perpendicular.md) | Slide or fix a contact on a curve, draw from a free point, or find a common perpendicular between two curves. |
| [`Normal`](line-normal.md) | Choose a curve or surface, locate the start on it, then constrain the endpoint along its normal. |
| [`Extension`](line-extension.md) | Pick a curve near an end, then project the endpoint onto that end's tangent. |

In the point-based construction modes and Extension, a positive number locks the length and a
negative number sets a length increment. Then pick or type a point to choose
the direction. For example, `Line Vertical 1,2,3 5 8,9,10` ends at `(1,2,8)`.
`Line Angled 1,2,3 5,2,3 30 10,10,3` rotates the reference line by 30 degrees
before resolving the last point. Ordinary Line retains the existing drafting
distance and angle shortcuts, coordinate filters, snapping and relative input.

Prompts, axis picking and viewport previews share the command geometry model.
Reference points remain outside the document until completion. Enter or Escape
cancels an incomplete line (Extension can accept a locked length on Enter); repeated points retain the prompt. A completed line
creates one Undo/Redo entry on the current layer. Most construction modes
preserve existing selection; Extension and Normal clear it while choosing a reference. `ReadCommandFile` uses the same getters and groups its edits; a
canceled line stops the file before subsequent commands execute.

Line admission is independent of document modelling tolerance. Native Rhino
accepts very short nonzero lines, including some that its own SDK marks invalid.
The public OpenNURBS length arithmetic and tiny-length unit-domain fallback are
retained. Finite constraint endpoints also matter near coordinates of 10¹²;
normalizing a reference too early changes the observed geometry.

The [qualification record](../line-construction-validation.json) covers 68 native
observations, including 54 complete workflows. Fourteen earlier recipes remain
unqualified input-driver diagnostics. Three complete tiny-line outputs are
native SDK-invalid: their coefficients and history are compared, while their
validity and 3DM round trips remain unqualified. Each build checks 40 valid
getter/file constructions and 37 valid direct commands through 3DM, plus
cancellation and atomic history. Coefficients, domains and 33 locally evaluated
samples use the same `1e-6` comparison gate.

Normal and Perpendicular have their own [normal](line-normal.md) and
[perpendicular](line-perpendicular.md) input rules and qualification. `Tangent` remains unsupported.
Independent native point sampling, continuous distance, arbitrary mouse
trajectories and comparative speed remain unqualified. Run the windowless
checks with `python3 -m tools.rhino_oracle.check_line_construction`.
