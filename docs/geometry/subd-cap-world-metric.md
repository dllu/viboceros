# SubD cap triangulation in world coordinates

[Cap usage](../commands/cap.md) · [Original comparisons](subd-cap-native.md) ·
[Current validation](subd-cap-world-metric-validation.json)

Dropping a coordinate projects a planar polygon into a valid two-dimensional
chart but can distort distances. The previous SubD cap used that chart's
Delaunay diagonals. On oblique planes this changed the control-face graph and,
with a smooth boundary, the limit surface. Independent tilted holdouts exposed
limit-vertex errors up to `1.31976969350556` units.

The shared constrained triangulator still validates region boundaries and holes.
Concave and holed SubD caps then legalize interior diagonals with their original three-dimensional
squared distances. The chart supplies orientation only. Outward interval
arithmetic certifies signs; uncertain signs, overflow and underflow use exact
binary64 rational inputs. A strictly illegal diagonal flips only when its two
incident triangles form a convex quadrilateral. Boundary edges stay fixed.
Exact circle ties retain their existing layout.

The queue deduplicates edges, bounding its storage by the current triangulation.
Refinement shares the existing planning work limit. A failed refinement leaves
the original SubD and the complete command transaction unchanged. Control
positions, native component identities, face data, planarity tolerance, and the
copy/history policies are retained.

The native matrices add [20 annular holdouts](../../tools/rhino_oracle/fixtures/subd_cap_annuli.json)
and [12 affine metric holdouts](../../tools/rhino_oracle/fixtures/subd_cap_metric.json)
to the original 26 cases. Inputs are independently authored; successful captures,
source archives and producer snapshots remain immutable. Command and application
tests compare all five lifecycle stages. Exact archive and target-mesh bytes
cache repeated surface sampling; other geometry and scene checks still run at
every stage. The coordinate bound remains `1e-6`.

All twelve metric holdouts agree in control-face graph and limit vertices after
the correction. The maximum measured limit-vertex difference is approximately
`7.1e-15`. Independent tests check an oblique rhombus, exact circle ties, boundary
retention, resource failure, and 480 circle signs against a separate full-matrix
rational determinant over five scale exponents.

Seven new smooth symmetric cases retain limit-surface differences associated
with cocircular diagonal choices. Fourteen new cases retain different face and
interior-edge graphs; face-cycle property pairing is unavailable there. The
[gap list](../../tools/rhino_oracle/fixtures/subd_cap_world_retained_gaps.json)
keeps each failed comparison explicit. The original two annular graph gaps also
remain. General Native triangulation tie behavior and complete Cap parity remain
open. Finite limit-mesh samples do not certify a continuous-surface error bound;
release and Rhino performance comparisons remain unqualified.
