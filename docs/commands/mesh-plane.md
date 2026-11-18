# MeshPlane

`MeshPlane` creates a rectangular mesh of quadrilateral faces. Pick two
corners or choose `Center`, `3Point`, `Vertical`, or `AroundCurve` from the
first prompt. `3Point` offers `EdgeMidpoint` before you pick the edge.
`AroundCurve` takes a curve or surface edge, then a center on that guide.
Hold Shift while picking a corner to make a square in the corner or center
mode. [Rhino MeshPlane](https://docs.mcneel.com/rhino/8/help/en-us/commands/meshplane.htm)

Set `XCount` and `YCount` before picking the first point, or type a positive
integer to set both. The initial setting is 10 faces in each direction. When a
count prompt is open, Enter keeps the displayed value. Accepted counts carry
over to the next run. Type `0` at the first point prompt to use the construction
plane origin.

The viewport draws the quad grid as you size the rectangle. Dense previews
skip interior grid lines. The finished mesh uses all requested faces and is
added in one Undo step.

```text
MeshPlane XCount=8 YCount=5 0,0,0 8,5,0
MeshPlane Center 0,0,0 4,3,0
MeshPlane 3Point EdgeMidpoint 0,0,0 4,0,0 4,3,0
MeshPlane Vertical 0,0,0 8,0,0 8,0,5
```
