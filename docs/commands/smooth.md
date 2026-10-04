# Smooth

[Command reference](README.md) · [Geometry rules and oracle evidence](../smooth.md)

Averages curve and surface control points or mesh vertices along enabled axes.
Select curves, single-face surfaces/B-reps, meshes, or their displayed grips.
Points and polysurfaces are excluded. Selected grips take precedence over a
selected parent.

```text
Smooth SmoothFactor=0.2 Steps=1 CoordinateSystem=World X=Yes Y=Yes Z=Yes FixBoundaries=Yes
```

- `SmoothFactor`: finite movement toward the neighbor mean; negative values
  move away and values above one can overshoot. Default `0.2`.
- `Steps`: positive integer passes, up to `2147483647`. Default `1`.
- `CoordinateSystem`: `World`, active `CPlane`, or local `Object` directions.
- `X`, `Y`, `Z`: enabled axes. Default `Yes` for each.
- `FixBoundaries`: holds open boundaries and the original closing/periodic
  control seam. Default `Yes`.

Start Smooth with a selection, or pick objects/grips and press Enter. Set
options, then press Enter to apply. Bare `X`, `Y`, `Z`, and `FixBoundaries`
toggle their current value. Bare `SmoothFactor`, `Steps`, or `CoordinateSystem`
opens its value prompt; Enter keeps the displayed value.

Escape cancels and discards pending options. Completed options are remembered
across documents in the same application and remain remembered after Undo.
A typed `_Cancel` at the main options prompt applies the edit, matching the
native scripted getter; at a value prompt it cancels.

One Undo restores geometry, attributes, grip state, and object order.
After completion, repeated Cancel clears selection and turns off grip display. Curve
primitives and polycurves promote to NURBS, including unchanged control nets.
Geometry user text is dropped from curves and untrimmed surfaces; meshes and
the tested circular-trim surface retain it. Object attributes remain intact.
If an Object edit separates periodic aliases, displayed grips grow and their
selection is cleared.

Nonrectangular trim images are fitted and checked at document tolerance, with
a ceiling of 4096 controls per spatial edge image.
Their spatial edge controls can differ from Rhino. SubD, edit-point and
subobject workflows, general singular trim changes, and performance parity
remain unverified.
