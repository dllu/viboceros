# Catenary

Start `Catenary`, pick the start and end, then pick an axis direction from the
start. Enter at the axis getter accepts the arrow along the initial CPlane's
negative Y direction. The axis and endpoint chord must define a plane.
The command uses the shared snapping, point filters, document units and typed
world/CPlane/relative-coordinate input.

`Mode` chooses the final constraint:

| Mode | Final input |
| --- | --- |
| ThroughPoint | A point on the catenary, projected into its plane |
| Length | Analytic chain length, greater than the endpoint distance |
| Parameter | The positive magnitude of `a` in `y = a cosh(x/a)` |
| Apex | A point specifying the apex height along the axis |

Enter `Mode=Length`, or enter `Mode` and then `Length`. Options remain available
between picks. A through point chooses the sag side relative to the endpoint
chord. An apex pick can reverse the axis to put the apex on the chosen side.
Length and Parameter accept document length units, signed numbers, or a single
point measuring distance from the start. Negative numbers reverse the sag;
picked numeric constraints select the side using their displacement from the
start along the axis.

The axis getter and numeric-constraint point getters resolve relative coordinates
from the start. ThroughPoint and Apex use the last accepted point, normally the
axis-direction pick. Relative vectors use the active CPlane axes. The captured
construction plane still determines the default axis and preview plane.

`Output=Smooth|Polyline`, `PointCount=integer`, and `MarkApex=Yes|No` control the
result. Bare `Output` and `MarkApex` toggle their values. `PointCount` also opens
a separate numeric getter. Commands accept 4 through 258 output points; smooth
output is cubic and polyline output has the requested vertex count. The initial
defaults are ThroughPoint, Smooth, 20 points and no apex marker. MarkApex adds an
analytic apex point before the curve; it need not lie on the approximated
segment. Both objects form one undo operation.

All four successful modes remember both analytic length and parameter, plus the
mode and output settings, within the command registry. Undo/Redo preserves these
settings. Length and Parameter initially have no numeric default: Enter cancels
until a successful construction has supplied a remembered value. Enter at a
numeric option keeps that option's value. Enter at the ThroughPoint getter can
put the apex at the second endpoint when that endpoint lies down the chosen
axis. Otherwise Enter cancels. Apex-mode Enter also cancels.

Previews and apex guides create no model objects. Invalid input keeps the getter
available for a retry. Escape, Cancel or another command abandons the pending
construction; cancelled local changes do not save preferences.

Existing complete script forms remain available:

```text
Catenary 0,0,0 10,0,0 0,0,-1 3,0,-2 PointCount=8
Catenary 0,0,0 10,0,0 0,0,-1 14 Mode=Length MarkApex=Yes
Catenary 0,0,0 10,0,0 0,0,-1 -4 Mode=Parameter Output=Polyline
Catenary 0,0,0 10,0,-2 0,0,-1 7,4,-4 Mode=Apex PointCount=7
```

The third point specifies the axis direction from the first. Full scripts retain
their explicit positional syntax and initial option defaults. Signed constraints
and picked sag sides also work in scripts.

## Verification and limits

[The validation record](../catenary-validation.json) contains 36 successful
Rhino 8.32 getter workflows. Frontend and command-oracle tests compare complete
controls, weights, knots, domains, lengths, apex markers, selection and actual
Undo/Redo at `1e-6`, with both geometry backends. Seven additional native
cancellations/failures and two ignored point-count inputs are retained as
separate input diagnostics.

Twelve public SDK/constructor probes verify the four-control smooth minimum and
two-point cubic tangent handling. For two interpolation points, each tangent
handle is one third of the chord length times `1 + sin²(angle to chord)`, with
the angle capped at 90 degrees. Longer interpolation sequences retain the
ordinary adjacent-chord handle policy. The earlier nine catenary cases remain
regression inputs.

Endpoint-snap-specific apex completion is not implemented; Enter provides the
verified endpoint-apex path. Full physical mouse/snap parity, native preference
changes on cancellation, extreme parameter ranges and performance parity remain
unqualified. Typed endpoint coordinates are a through-point request and can
fail; they do not carry Rhino's endpoint-snap provenance.

Reference: McNeel's public
[Catenary command documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/catenary.htm).
