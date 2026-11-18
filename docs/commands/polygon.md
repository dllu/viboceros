# Polygon

`Polygon` creates a closed polyline. Set `NumSides` at any point prompt.
`Mode=Inscribed` takes a center and a vertex. `Mode=Circumscribed` takes a
center and an edge midpoint. Entering `Mode` switches between the two.
The side count starts at five and carries over with the mode.

`Edge` takes the endpoints of one side. Type `Flip` at the second prompt
to place the polygon on the other side of the edge. `Star` takes a center,
an outer corner, and an inner radius. Press Enter at the inner-radius prompt
to calculate that radius automatically.

Choose `Vertical` before the first point for a polygon perpendicular to
the construction plane. Choose `Edge` or `Star` first to use it with those
modes. `AroundCurve` takes a curve or surface edge and a center on that guide.
It puts a radial polygon or star in the plane perpendicular to the guide.
[Rhino Polygon](https://docs.mcneel.com/rhino/8/help/en-us/commands/polygon.htm)

The outline follows the cursor after the first point. Type a size to lock
the radius, apothem, or edge length, then pick its direction. Enter uses the
current direction. A negative outer size locks twice its magnitude. An inner
radius is accepted immediately. The viewport samples the outline when there
are more than 256 sides. The polyline uses all requested sides and has one
Undo step.

```text
Polygon 6 0,0,0 4
Polygon NumSides=5 Mode=Circumscribed 0,0,0 4,0,0
Polygon NumSides=6 Edge 0,0,0 Flip 6,0,0
Polygon NumSides=5 Star Vertical 0,0,0 5,0,3 Enter
```
