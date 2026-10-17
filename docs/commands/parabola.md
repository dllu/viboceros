# Parabola and Parabola3Pt

Start either command without coordinates to pick points in the viewports. Both
commands use the common typed-point, relative-coordinate, unit, point-filter,
object-snap and drafting input. Their curve and focus previews do not edit the
model. A completed curve and optional focus point form one undo operation;
Escape, Cancel or Enter abandon an unfinished construction.

`Parabola` starts with `Focus`: pick a focus, opening direction, then an end.
Choose `Vertex` before the first pick to specify a vertex, focus, then an end.
Vertex mode uses the end's distance perpendicular to the axis to determine its
height. `Half` draws only the vertex-to-end half; `MarkFocus` adds a point object
before the curve. Both options accept `Yes`/`No` or toggle when entered alone,
and can change between point picks.

`Parabola3Pt` offers three modes:

| Mode | Point in addition to the two endpoints | Final input |
| --- | --- | --- |
| ThroughPoint (initial default) | A point on the parabola | Opening direction |
| Focus | Focus | None |
| Vertex | Vertex | None |

`PickOrder=Default` picks start, special point, end. `EndsFirst` picks start,
end, special point. `EndsLast` picks special point, start, end. Choose the order
before the first pick. `Mode` may change before the third point is accepted;
`MarkFocus` also remains available at the opening-direction prompt. Enter
`Mode=Focus` directly, or enter `Mode` followed by `Focus`; `PickOrder` has the
same two input forms.

The opening-direction vector starts at the **last of the three picked points**.
For Default and EndsLast this is the end; for EndsFirst it is the special point.
The direction getter uses the three points' plane for mouse input. Typed world
coordinates remain explicit three-dimensional points.

```text
Parabola Vertex 0,0,0 0,0,1 4,0,99 Half=Yes MarkFocus=Yes
Parabola3Pt Mode=ThroughPoint PickOrder=Default -1,0,0.25 1,0,0.25 3,0,2.25 3,0,3.25 MarkFocus=Yes
Parabola3Pt Mode=Vertex PickOrder=EndsFirst -1,0,0.25 3,0,2.25 0,0,0 MarkFocus=No
```

Each command remembers its settings after successful execution, including
across undo/redo and documents sharing a registry. A cancelled or failed local
construction does not save settings. Invalid picks leave the current getter
available for a corrected pick. The implementation reuses the existing exact
quadratic constructors and the Paraboloid axis/dimension calculation.

## Verification and limits

[The validation record](../parabola-validation.json) records 24 successful public
Rhino 8 command workflows: Focus/Vertex, full/half, focus-marker object order,
all nine three-point mode/order combinations, options between picks, remembered
settings and oblique inputs. Exact controls, weights, knots, domain, length,
selection and actual undo/redo are compared at absolute epsilon `1e-6`, with
both geometry backends. Frontend tests replay the same authored inputs.

Three retained native direction controls report Success with no output when the
wrong direction origin points the parabola the wrong way. Viboceros rejects
these directions without editing the model and allows another pick. These are
not counted among the 24 successful geometry matches. Changing PickOrder after
a pick, changing the axis construction after its first pick, off-plane typed
opening-direction projection, exhaustive snap behavior, and performance parity
have not been qualified.

The documented Rhino construction modes are described in
[McNeel's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/parabola.htm).
