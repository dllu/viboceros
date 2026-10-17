# Arc Deformable

Start `Arc`, choose `Deformable`, then construct the arc using Center,
StartPoint, Midpoint, Tilted or Tangent. The result is an editable,
non-rational NURBS curve. Ordinary Arc starts with exact circular output.

`Degree` accepts 2 through 11. `PointCount` accepts degree + 2 through 1000.
Both support an integer getter or `Name=value`. Enter accepts a valid saved
value; invalid numbers keep the getter open. The initial settings are degree
3 and 10 points. Successful construction saves both settings across documents
during the application session. Cancellation and failed construction discard
pending changes; undo and redo affect the model while retaining preferences.

Option order matters. A count set before raising the degree remains in place.
For example, degree 3 with four points is available through:

```text
Arc Deformable Degree=2 PointCount=4 Degree=3 0,0,0 4,0,0 90
Arc Deformable Degree=5 PointCount=8 StartPoint 4,0,0 ThroughPoint 0,4,0 -4,0,0
Arc Deformable Degree=3 PointCount=10 Center 0,0,0 Tilted 4,0,0 0,3,3 -90
```

The retained count must exceed the degree for construction to succeed.
In native Deformable Midpoint numeric construction, the supplied radial
point sets the curve's start seam. This differs from exact Arc Midpoint,
which centers the sweep around that point.
Degree and point count remain independent of Tangent's saved radius.
Center-based length input accepts a signed number or two spatial distance
points. `Direction=Clockwise` reverses the length direction. The arc keeps
its original construction plane when another viewport supplies distance points.

Open arcs use the shared uniform Rebuild solver with projected endpoint
handles and an arc-length domain. Full circles use Circle's existing
periodic or clamped layouts and angular domain, so their control count can
exceed PointCount. The circle classification uses OpenNURBS' angular
tolerance. Viewport previews use the same output conversion as completion.

[Validation](../arc-deformable-validation.json) records native Rhino input
receipts, complete controls, weights, knots, domains and undo/redo comparisons.
The 1e-6 comparison tolerance measures agreement with Rhino's approximation;
it does not bound the approximation's distance from an ideal circle.
Raw rejected input and incomplete captures are retained separately from the
qualified workflow profiles. Midpoint endpoint parity remains unqualified:
the native typed-endpoint capture timed out. The local point API and preview
use the existing exact midpoint construction before approximation.
Arc Extension remains pending.
