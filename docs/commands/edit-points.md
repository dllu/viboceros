# EditPtOn

Select a curve and run `EditPtOn`, or run the command first and pick curves.
The displayed points lie on the curve at its Greville parameters. Click or
window-select them and use Move, Copy, Scale, Scale1D, Scale2D, ScaleNU, Rotate,
Rotate3D, Mirror or Shear through the existing
[grip transform workflow](../grip-transforms.md). `PointsOff` hides both control
and edit points. `PointsOn` switches an owner back to its control points;
switching modes clears that owner's grip selection.

For example, create `ControlPointCurve 2 0,0,0 1,3,0 3,-2,0 4,0,0`, select it,
run `EditPtOn`, select an interior edit point and run `Move 0,0,0 0,2,0`.
The moved station shifts two units in Y while the other edit stations retain
their locations. The intervening curve and its control points can change.

Displaying edit points preserves the source geometry, Undo and Redo. An edit
reconstructs the curve with its original degree, knot vector, control count,
parameter domain and rational weights. The shared original-basis interpolation
solver and exact rational station checks from
[Loose Project/Pull](projection-loose.md) validate the requested locations at
document tolerance. Inputs and outputs are staged before document mutation.
Edited curve primitives become NURBS curves. Copy edits a duplicate owner;
Undo/Redo exchange the geometry, grip mode, display and selected indices using
the existing grip history policy. All viewports share the same preview geometry;
the preview cache includes grip mode even when the source geometry is unchanged.
Whole-object `ScalePositions Copy=Yes` also retains the copied owner's grip mode
and selected indices.

Supported layouts are clamped open or closed NURBS and periodic NURBS with exact
repeated tails, degree at most 17 and at most 1000 stored controls. Weights must
have a common sign. Periodic edit points omit repeated tail stations. Full-order
knots retain separate left/right stations. Unsupported or ill-conditioned
layouts decline without a partial edit. Smooth with selected edit grips also
declines; switch to `PointsOn` to smooth controls. SetPt does not yet accept grips.

This is a curve implementation. SubD edit points, closed-curve seam aliases,
native grip ordering and exact Rhino editing behavior need qualification.
McNeel's [EditPtOn reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/pointson.htm#EditPtOn)
describes curve and SubD display. No new live Rhino comparison or performance
qualification is claimed. Live checks were deferred because of the recorded
[Wine SSPI startup issue](../oracle-sspi.md).

Implementation regressions cover rational and periodic partial edits, preserved
structure, atomic failure and transaction rollback, mode switching, Copy,
Undo/Redo, direct and incremental Move input, command-first curve selection,
Smooth rejection and shared preview invalidation.
The [qualification record](../edit-points-qualification.json) retains the source
hashes, check results and unqualified behavior.

```sh
cargo test -p viboceros-document control_points
cargo test -p viboceros edit_points
cargo test -p viboceros grip_preview
cargo test -p viboceros grip_transform
```
