# Certified B-rep face partitioning

[Architecture](architecture.md) · [Edge cleanup](join-edge-cleanup.md) · [Mass properties](mass-properties.md)

`Brep::try_split_face_at_knot` partitions a selected face along one continuous
interior U or V knot of full degree multiplicity. `try_split_kinky_faces`
repeatedly partitions qualifying surface creases. These are kernel APIs;
replacement-time splitting is not yet wired into `MergeAllEdges`.

## Preservation and topology

Tensor patches copy control points, weights and retained knot intervals exactly.
No surface is fitted. Boundary subdivision updates every incident trim, including
neighbors and closed seams. Existing vertices and first edge slots remain in
place; new vertices, edges and additional faces are appended. Face senses and
existing component uncertainties are preserved.

Root finding and closest-point searches only propose subdivisions. Acceptance
requires exact spatial and UV curve-locus certificates: zero-displacement curve
restrictions, or an ordered chain of exact, gap-free, monotone collinear segments.
The latter preserves geometry, not necessarily the source curve's parameter speed.
A rounded line endpoint may be replaced by its source evaluation only when the
resulting entire partition passes this certificate. Uncertifiable approximations
are errors, not a tolerance-based substitute for UV preservation.

Sign-coherent rational control hulls certify each trim's half-plane membership.
Oriented incidence connects cut endpoints and traces loops; spatial vertex
identity is kept separate from UV identity across closed seams. Clipping can open
holes into outer boundaries or create multiple disconnected faces. Untouched
holes must have one containing output face, using the existing containment
predicate. Boundary, mated and seam labels are recomputed from face ownership.
New spatial isocurves carry a certified construction-error bound; this bound must
not exceed model absolute tolerance.

## Limits

- General curve certificates and crossing proposals support degree 16.
- Mixed-sign trim hulls, unresolved or unrepresentable exact restrictions,
  ambiguous/touching incidence, crossing singular trims, discontinuous full-order
  surface knots and collapsed new interior seams are rejected atomically.
- A trim whose hull straddles the cut can be rejected even if its curve does not
  cross. Arbitrary curved-region Boolean splitting is not implemented.
- Angular detection samples one-sided isocurve tangents at transverse span ends
  and midpoints. It follows the existing surface candidate API, not a continuous
  maximum-angle certificate.
- All face partitions share 16 million charged work units. Tensor copies,
  angular-scan multiplicative cost, root proposals and exact certificates are
  charged. The edge-subdivision helper also retains its separate four-million
  per-call budget. Crossing and per-side trim counts are limited to 100,000.
- Ordinary B-rep validation and its documented containment limitations still
  apply. No arbitrary-input Rhino equivalence or performance ratio is claimed.

## Rhino observations

[107 fixtures](../tools/rhino_oracle/fixtures/merge_edges_face_splits.json),
[raw observations](../tools/rhino_oracle/observations/merge_edges_face_splits.json)
and [provenance](face-splitting-provenance.json) retain two live Rhino
8.32.26160.13001 probes on 41 shared, round-trip-checked 3DM sources. Runs used an
owned private Xvfb, not the user's desktop Rhino. There are 54 angular probes and
53 upper-cutoff/layout probes, including both UV axes, face senses, shifted
domains, prior boundary splits, and Undo/Redo. These are correctness observations,
not timings.

The measurements support a separate **0.1°–2° face-splitting cutoff**, versus
**0.1°–1° for edge merging**. At a 5° document tolerance, a 1.9° crease remains
one face while a 2° crease becomes two. The upper bracket includes 1.9999°,
2°, and 2.0001° at document tolerances 1.5°, 2.5°, and 10°. The retained curve
domains show cleanup before face splitting: newly split boundary segments keep
their source parameter intervals instead of receiving another straight-edge
cleanup pass.

The kernel replay applies edge cleanup followed by face partitioning and matches
**104/107 complete raw output geometry records** at absolute epsilon `1e-9` and
relative epsilon `1e-10`. It retains all surface/curve definitions, component
uncertainties, face order, trim order and parameter intervals without normalization.
Three exact 2° cases differ: native stable `atan2` is at or just below the cutoff
while Rhino splits. Tests explicitly retain this topology disagreement and bound
the angular discrepancy below `1e-14` radians. This kernel replay does not yet
establish command selection, replacement or history equivalence for face splitting.

An initial extended source export used a parameter outside a localized edge's
domain. It was corrected before exporting fresh artifacts; that failed attempt
is recorded in provenance and is not counted as a Rhino comparison.

## Native validation

Focused tests cover shared neighbors, holes, disconnected clipped regions,
quadratic UV crossings, closed seams, reversed faces, repeated U/V splits,
out-of-region knots, uncertainty preservation, UV origins `±2^40`, UV scale
`2^-30`, signed/extreme weight gauges, unclamped knots, atomic rejection, and
work-budget exhaustion. A closed cylinder remains solid after partitioning its
side, updates both caps, and round-trips through 3DM with exactly equal geometry
and topology. Its area and signed volume agree with analytic values.

That cylinder exposed an existing integration defect: subdividing a cap boundary
switches it from rectangular integration to planar boundary integration. The
planar path previously skipped surface-knot crossings even though a planar
surface can have a non-affine UV map. Planar and nonplanar boundary integration
now share crossing isolation and per-face interval/evaluation limits. Independent
edge-only subdivision tests reproduce and guard this fix without face splitting.

```sh
cargo test -p viboceros-geometry brep::face_split
cargo test -p viboceros-geometry brep::mass_properties
cargo test -p viboceros-io partitioned_cylinder_faces
cargo test -p viboceros-oracle partition_kernel_replays
```

Verification checkpoint: 2,929 release-mode workspace tests, seven opt-in GPU
tests, 237 Python tests, formatting, and Clippy/Rustdoc with warnings denied.
