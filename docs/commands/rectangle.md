# Rectangle

Enter `Rectangle` (`Rect`), pick the first and opposite corners, and inspect the live outline before completing the rectangle. The two-corner form uses the first pick's construction plane and elevation. Switching viewports after that pick preserves its orientation. Escape discards the draft; completion adds one closed polyline with one Undo/Redo step.

At the first prompt, choose `Center`, `3Point`, `Vertical`, or `AroundCurve`:

- `Center`: pick the center and a corner. Numeric length and width are full dimensions.
- `3Point`: pick the start and end of an edge, then a width point. The edge and width point can define an arbitrary 3D plane. A typed edge length constrains the next direction pick.
- `EdgeMidpoint`, within 3Point: pick the first edge's midpoint and one endpoint, then its width point. A typed edge length measures from midpoint to endpoint.
- `Vertical`: pick an edge and a height point. The rectangle's plane contains the edge and the captured construction plane's Z direction. Height is measured perpendicular to the edge in that plane.
- `AroundCurve`: select a curve or surface edge, then pick its center location. The rectangle is centered at the closest curve point, perpendicular to the tangent, and takes the same corner/dimension inputs as Center.

After the first corner, `3Point` retains that corner. After a center, `3Point` takes an edge midpoint and a width point, producing a rectangle symmetric about the center. Numeric widths in 3Point and Vertical mode require a final side/orientation pick. Enter at the width prompt uses the measured length (midpoint to endpoint in EdgeMidpoint mode) and still requires that pick. In Corners and Center mode, entering a length and pressing Enter for width completes a square along the construction axes. Interactive dimensions accept the existing calculator and length units.

One-line coordinates are world coordinates. Examples:

```text
Rectangle 0,0,0 10,6,0
Rectangle Center 1,2,3 10 6
Rectangle 3Point 1,2,3 4,6,8 7,-1,3
Rectangle 3Point EdgeMidpoint 1,2,3 10 4,6,8 7,-1,3
Rectangle Vertical 1,2,3 5,8,9 5 1,2,-8
Rectangle AroundCurve Curve=12 0,0,5 8 6
Rectangle AroundCurve Edge=13,0 0,0,0 8 6
```

The same getter supplies [FlowAlongSrf's temporary Plane](flow-along-surface.md), without adding construction objects. Captured curve/edge references must remain selectable and unchanged until the center is accepted. Invalid dimensions remain correctable. Rounded rectangles are not implemented. AroundCurve uses explicit curve/edge picking; Rhino’s automatic acceptance of preselected source curves and its grip-selection side effects are outside the qualified cases.

The [rectangle validation](../surface-flow-rectangles-validation.json) compares 60 completed native Rectangle outlines and FlowAlongSrf workflows, including numeric inputs, arbitrary planes, curved guides, copies, rigid groups, grips and actual Undo/Redo, at `1e-6`.

Public reference: [Rhino Rectangle](https://docs.mcneel.com/rhino/8/help/en-us/commands/rectangle.htm).
