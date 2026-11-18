# TruncatedPyramid

Run `TruncatedPyramid` and draw the base. The inscribed, circumscribed, edge,
and star inputs follow the [Pyramid base prompts](pyramid.md). Set
`NumSides` before picking the base, and use `Solid=Yes` for caps on both rims.

Pick the top center or enter a signed height. `DirectionConstraint=Vertical`
uses the base normal. `DirectionConstraint=AroundCurve` asks for a guide and
a location on it, then uses the tangent there. With
`DirectionConstraint=None`, the pointer sets the base orientation, and
the accepted endpoint sets the top center. A complete script uses the
endpoint for orientation when there is no viewport cursor.

At the final **Pick point** prompt, move the pointer to resize the live
top ring, then click or enter a positive size. The top is sized along a line
through its center parallel to the base’s defining direction. Inscribed sizes
measure to a corner, and circumscribed and edge sizes measure to the midpoint
of an edge.
A numeric size of zero or less ends the command without making an object.

```text
TruncatedPyramid NumSides=5 DirectionConstraint=Vertical w1,2,3 w5,2,3 w1,2,9 2
```

Existing positional scripts can still specify both circumradii and an
explicit axis.

```text
TruncatedPyramid 5 0,0,0 5 10 2.5 Axis=0,0,1 Solid=Yes
```

The star preview and output scale both star radii together. The Rhino 8
star cases checked so far stayed at the final prompt after top sizing.

[Surface commands](surfaces-index.md)
