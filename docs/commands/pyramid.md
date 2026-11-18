# Pyramid

Run `Pyramid`, choose the base construction, and pick or type the requested
points. `NumSides` sets the number of polygon sides or outer star corners.

| Base | Input |
| --- | --- |
| `Mode=Inscribed` | Pick the center and a vertex. |
| `Mode=Circumscribed` | Pick the center and the midpoint of an edge. |
| `Edge` | Pick both ends of a base edge. Use `Flip` to place the rest of the base on the other side of that edge. |
| `Star` | Pick the center and an outer corner, then give the second radius. Press Enter for the automatic inner radius. |

`DirectionConstraint=Vertical` uses the base normal for the apex.
`DirectionConstraint=None` lets you move the apex freely and reorients the
base as you move it. For a star, the cursor at the second-radius prompt sets
the orientation used during the apex preview. A complete script uses a
deterministic orientation in the absence of a viewport cursor. Off-plane
radius inputs in free-direction stars can produce different radii and
orientation in Rhino.
`DirectionConstraint=AroundCurve` asks for a guide and a
location on it. The tangent at that location gives the base normal. The
`Edge` base offers `Vertical` and `None` directions.
Pick the apex or type a height. The viewport draws the base and then the
apex edges before the command adds geometry. `Solid=Yes` caps the base.

The following complete command uses the circumscribed getter and a picked apex.

```text
Pyramid NumSides=6 Mode=Circumscribed DirectionConstraint=Vertical w1,2,3 w5,2,3 w1,2,8
```

The positional form still accepts a side count, center, radius, and signed height.

```text
Pyramid 5 0,0,0 3 5 Axis=0,0,1 Solid=Yes
```

[Surface commands](surfaces-index.md)
