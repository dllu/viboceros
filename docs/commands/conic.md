# Conic

Run `Conic`, pick the start, end and apex, then pick a curvature point or enter
rho between zero and one. The viewport previews the exact rational quadratic.
`Apex` after the start changes the order to start, apex, end; `Vertical` then
constrains the end to the line through the apex parallel to the captured CPlane Z.

```text
Conic 0,0,0 10,0,0 4,6,0 0.5
Conic 0,0,0 Apex 4,6,0 10,0,0 0.75
Conic 1,2,3 Apex 4,6,8 Vertical 11,9,4 0.5
Conic 0,0,0 10,0,0 4,6,0 4,2,3
```

The last example projects its curvature point into the conic plane. Rho below
0.5 gives an elliptic segment, 0.5 a parabola, and above 0.5 a hyperbolic segment.
Numeric rho retains collinear and coincident controls. A failed shape-point
solution ends the command without adding geometry; an invalid numeric rho can
be corrected. Enter accepts the last successful rho in this command registry;
there is no default before the first successful conic.

At either endpoint, choose `Tangent` or `Perpendicular`, select a curve or surface
edge, and pick the endpoint on it. Tangent constrains the apex to the endpoint's
3D tangent line. Perpendicular uses the line perpendicular to that tangent and
the active CPlane normal. A tangent parallel to CPlane Z gives no perpendicular
apex constraint. With two endpoint constraints, their intersection supplies the
apex; parallel or skew lines fail without creating an object.

Scripted selections use object IDs:

```text
Conic Tangent Curve=12 2,0,0 10,8,0 5,3,0 0.5
Conic 0,0,0 Perpendicular Edge=27,0 10,5,0 4,8,0 0.5
Conic Tangent Curve=12 2,0,0 Tangent Curve=13 10,5,0 0.5
```

`SubCrv` before selecting the reference opens a temporary start/end picker. The
endpoint then stays on that portion. Move along the curve and enter `D` or
`Direction=Locked` to keep that direction; `Direction=Free` releases it. A point
on the opposite side of a locked open curve returns to curve selection.

Locked numeric lengths finish immediately on open curves and clamp at the
source end. Free lengths and all closed-curve lengths take a confirmation point;
on a closed curve that point can choose either direction. Zero length and Enter
inside the temporary picker return to curve selection, preserving the conic's
existing endpoint. The local `FullCurve` alias restores the captured whole
curve directly.

Sources and surface edges remain unchanged. Previews reuse cached geometry and
add no helper objects. Hidden, deleted or replaced references must be picked
again. Esc discards the unfinished construction; completion is one undoable edit.

```text
Conic Tangent SubCrv Curve=12 2,0,0 6,0,0 9,2,0 10,8,0 5,3,0 0.5
```

For scripted pointer-direction input, supply `Hover=point` before locking:

```text
Conic Tangent SubCrv Curve=12 6,0,0 Hover=8,0,0 D 3 9,1,0 10,8,0 5,3,0 0.5
```

The original [42-workflow validation](../conic-validation.json) and the
[28 getter workflows](../conic-getters-validation.json) check 70 Rhino 8 cases at
epsilon `1e-6`: complete NURBS form, length, closure, selection, source preservation
and actual Undo/Redo. The added cases use real owned-window pointer motion,
acknowledged by Rhino's `OnEndMouseMove`, and physical surface-edge clicks. They
cover open/closed direction locks, confirmations on either side, seam crossings,
nonuniform polylines, custom CPlanes, both endpoint constraints and curved 3D
surface borders.

Native length queries use `1e-12` fractional precision; the original default-query
measurement on a backtracking collinear curve remains a separate control.
Automatic acceptance of preselected references, exhaustive snapping and
polysurface topology cases, extreme scales, and performance parity remain
unqualified.

Public reference: [Rhino Conic](https://docs.mcneel.com/rhino/8/help/en-us/commands/conic.htm)
and [SubCrv](https://docs.mcneel.com/rhino/8/help/en-us/commands/subcrv.htm).
