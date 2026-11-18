# Paraboloid

Start `Paraboloid` and pick or type points as prompted.

| Construction | Picks |
| --- | --- |
| Focus | Pick the focus, a point along the opening axis, and the end point on the rim. |
| Vertex and focus | Choose `Vertex`, then pick the vertex, the focus, and an end point. The end point sets the rim radius perpendicular to the axis. |
| Vertex and direction | Choose `Vertex`, pick the vertex, then choose `Direction`. Pick a point along the axis and an end point on the rim. The end point determines the focus and which way the surface opens. |

Move the pointer at the end prompt to preview the exact rim and meridians.
Distance locks and relative coordinates use the first pick as their origin.
With a picked vertex and focus, a distance at the end prompt sets the rim
radius. Free mouse picks use the direction in the active CPlane perpendicular
to the paraboloid axis.
At any prompt, `Solid=Yes` adds a planar rim cap and `MarkFocus=Yes` adds a
point at the focus. Both settings carry forward, including when you cancel.
Each new command starts in Focus mode. One Undo removes the completed surface
and its optional focus marker.

A complete staged invocation can keep options in the order you would enter
them interactively.

```text
Paraboloid Vertex w1,2,3 Direction w1,2,8 MarkFocus=Yes Solid=Yes w4,6,5
```

Short positional Focus and Vertex scripts keep their original defaults and
place any requested marker before the surface.

[Surface commands](surfaces-index.md)
