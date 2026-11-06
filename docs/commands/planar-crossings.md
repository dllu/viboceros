# Crossing PlanarSrf holes

PlanarSrf retains the original inner loops when seam parity admits two crossing
boundaries as holes of the same outer curve. The exact loops remain separate in
the B-rep and in 3DM output. Model area subtracts both original hole areas;
the displayed mesh excludes their union. A captured 100-unit rectangle with
two crossing 16-unit holes therefore has model area 68 and mesh area 72.
Reversing curves, changing their domains or transforming the scene preserves
the captured behavior. Moving a seam can change which loops become holes.

The explicit geometry constructor
`Brep::try_planar_face_with_overlapping_holes` validates each hole against its
entire outer boundary. The ordinary `try_planar_face_with_holes` keeps its
existing disjoint-hole validation. Native command builds still admit the
profiles through usd-brep and try native region construction first.

When ordinary trim triangulation cannot represent crossing siblings, the
mesher splits their sampled constraints at intersections. It classifies the
resulting regions against the union of the holes and audits material area
against the active constraint boundary. Source curves and UV trims are not
split or replaced. The fallback keeps the existing 131,072-vertex limit and
bounds constraint traversal and recovery to four million steps. It rejects
unsupported individual holes and outer-boundary crossings.

The [validation record](../planar-crossings-validation.json) adds 89 primary
native workflows: 48 independently moved seam pairs, eight mesh-coverage scenes,
and 33 orientation, parameter, homogeneous-gauge, polygon-refinement and affine
holdouts. Both builds replay the command, application history and output 3DM
archives. Eight scenes also compare 64 spatial coverage stations against Rhino's
default render meshes; polygonal mesh areas match within `1e-9`.

One tiny-circle scene retains a native area-reporting discrepancy of about
`1.09813e-9`. Tests record it explicitly and separately check the complete
positive NURBS boundary bases and analytic area. Native spatial boundaries and
affinely lifted UV trim controls match the source definitions within `1e-12`.
Another 66 native workflows confirm unchanged geometry and preserve the same
scalar discrepancy after translation and unit-domain reparameterization.

These are sampled mesh checks. Curved mesh intersections lie on sampled chords;
native tessellation topology, continuous curve-error bounds and arbitrary
self-intersections remain unqualified. Native face-membership diagnostics are
retained separately: some differ from the generated mesh's coverage and are not
used as mesh targets. Complete native B-rep parameterization and output birth
order remain outside the region comparison.

```sh
cargo test -p viboceros-oracle --test planar_crossings --test planar_regions --test planar_regions_archive
python3 -m unittest tools.rhino_oracle.test_planar_crossings
```
