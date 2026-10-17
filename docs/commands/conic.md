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
endpoint then stays on that portion. An entered length takes a confirmation
point to choose its side. Enter or the local `FullCurve` alias restores the
original captured curve. Sources and surface edges remain unchanged, and no
helper objects are added. Hidden, deleted or replaced references must be picked
again. Esc discards the unfinished construction; completion is one undoable edit.

```text
Conic Tangent SubCrv Curve=12 2,0,0 6,0,0 9,2,0 10,8,0 5,3,0 0.5
```

The [validation record](../conic-validation.json) covers 42 fresh Rhino 8 command
workflows at epsilon `1e-6`: full NURBS controls, weights, knots, domain, length,
closure, selection, source preservation and actual Undo/Redo. Native length
queries use the request's `1e-12` fractional precision; the original default-query
measurement on a backtracking collinear curve is retained as a separate control. It includes custom
CPlanes, 3D constraints, circular guides and a temporary portion crossing a
circle seam. Edge selection has local frontend regression coverage. Exhaustive
native edge interactions, locked SubCrv direction, and performance parity remain
unqualified.

Public reference: [Rhino Conic](https://docs.mcneel.com/rhino/8/help/en-us/commands/conic.htm).
