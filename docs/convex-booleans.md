# Convex polyhedral Boolean foundation

[Architecture](architecture.md) · [Development](development.md) · [Oracle](oracle.md)

The geometry kernel exposes `Brep::try_boolean_convex(other, operation, tolerance)`
with `BrepBooleanOperation::{Union, Intersection, Difference}`. Difference means
the first operand minus the second. Inputs remain unchanged. The returned
`Option<Brep>` is `None` for an empty region; a nonempty B-rep can contain separate
outer shells or an inward cavity.

The [BooleanUnion command](commands/boolean-union.md) now adds selection,
DeleteInput, remembered options, source metadata and history for the certified
convex polyhedral scope. [BooleanIntersection](commands/boolean-intersection.md)
adds common-set and two-set command workflows.
[BooleanDifference](commands/boolean-difference.md) adds separate target/cutter
sets and observed deletion, contact, metadata, and history policies.

## Supported inputs and construction

Each operand must be one closed manifold convex polyhedral shell. Faces must have
an exactly affine, degree-one 2×2 supporting surface with uniform weights, one
convex polygonal loop, and a separate straight two-control 3D edge for each trim.
Surface/vertex correspondence and half-space convexity are checked using the exact
rational values of stored binary64 coordinates. Inward shells are normalized.

Half-space clipping constructs convex face fragments. Coplanar overlap is owned
once; opposing contact patches are removed from a union. Exact shared vertex
subdivisions resolve T-junctions before edges are built. Supporting surfaces are
retained, and their inverse affine maps supply the new UV trims. Cartesian and UV
coordinates are rounded once before ordinary B-rep validation at the caller's
tolerance. Coplanar fragments remain separate faces.

Limits are 256 faces per input, 4,096 output fragments, two million units of exact
predicate/subdivision work, and 8,192 bits per constructed rational coordinate
or shell-volume accumulator.
Collapsed rounded vertices, unsupported input certificates, exhausted budgets,
and unresolved/nonmanifold output return errors. Curved, open, nonconvex,
multi-shell, singular-trim, and input-hole cases are unsupported. Exact affine
certification can reject control nets affected by ordinary transformation
roundoff; this API does not widen tolerance to accept them. Singular edge/point
contacts are rejected when they would join shells nonmanifoldly.

## Multiple operands and face merging

`union_convex_breps` accepts up to 128 original certified convex operands. Each
face is clipped against original bodies; a nonconvex intermediate is never
treated as convex. Exact signed shell volumes and retried exact ray containment
separate material components and attach each cavity to its innermost surrounding
outer shell. Nested material islands retain their own cavities. Reports retain
source indices, boundary contributors, and original face ownership. Strict
interior inputs are distinguished from boundary contributors for command
metadata. Coplanar overlap ownership uses earliest input order.

`try_merge_coplanar_polygon_faces_in_groups` combines eligible adjacent affine
planar polygon faces within caller-assigned groups. It removes interior edges,
reconstructs exterior and hole loops, extends an affine supporting surface when
needed, and keeps exterior edge curves. Collinear exterior subdivisions can be
removed with `try_merge_all_edges`. Ambiguous junctions fail explicitly. Kernel
tests cover multiple-body chains, contact and nesting classification, coplanar
faces and through-holes, enclosed cavities, and nested islands.

`intersect_convex_breps` clips each original face inside all other operands to
construct their common convex region, retaining original face ownership.
`intersect_convex_brep_sets` intersects every original cross-set pair exactly,
removes redundant contained pair regions, then unions those exact regions before
rounding once. Reports retain all nonempty pairs, maximal pairs, and original
face sources. Material components and cavities use the same decomposition as
union. Combined original input count is limited to 128, and total intermediate
pair fragments are limited to 4,096 in addition to the shared work/output limits.
The kernel implements mathematical nested/equal intersections; the native
command adapter applies its separately observed boundary-interaction policy.

`subtract_convex_breps` subtracts the union of original cutters from one target.
Exact target-clipped cutter regions are compared first to remove redundant
contained regions, retaining partial coplanar ownership. Target faces are
clipped outside every cutter; exposed cutter patches inside the
target reverse orientation to become the new material boundary. The shared
material decomposition separates disconnected remainders and attaches cavities
using exact shell volumes and exact ray containment on face fragments. It needs
no original-target interior witness, which may lie in removed material. Up to
127 cutters are accepted, with the same certificate, work, and fragment limits.
Original input/face ownership is retained. Native commands process targets
separately and ignore noninteracting enclosed cutters; the kernel computes the
mathematical difference and can retain enclosed voids.

`convex_brep_subtraction_interactions` includes positive-length boundary edge
contacts, in addition to the face interactions used by union and intersection.
Exact segment clipping distinguishes edge contacts from point-only contacts;
equivalent convex regions remain excluded.

## Verification

The kernel tests check overlaps, coplanar patches, face contacts, through-holes,
disjoint bodies, nested cavities, equal operands, inward orientations, an interval
grid with independent analytic volumes, tetrahedra, shear/reflection covariance,
and a cut with a nonrepresentable rational vertex. They also check input purity,
unsupported inputs, singular contacts, and bounded work.

A private Xvfb capture of Rhino **8.32.26160.13001** retains 36 public SDK calls:
12 fixed recipes × union/intersection/difference. The 26 calls returning nonempty
valid solids are replayed against the kernel. Comparisons use absolute epsilon
`1e-10` for volume, centroid and tight polygon vertex bounds, `1e-9` for area,
and `1e-7` for bidirectional boundary witnesses. Native vertices and nine stations
per edge are checked against kernel face regions; kernel vertices, edge midpoints
and face interior witnesses are checked against native regions. Different face
partitions and parameterizations are allowed. These finite witnesses do not
certify continuous boundary error or all input geometries.

The saved SDK response also contains six no-result cases where the mathematical
set is nonempty. The replay keeps these differences visible:

| Recipe / SDK operation | Kernel set result | Captured SDK result |
| --- | --- | --- |
| Disjoint / difference | First body | Empty array |
| Contained cutter / intersection | Inner body | Empty array |
| Contained cutter / difference | Outer shell with cavity | Empty array |
| Containing cutter / intersection | First body | Empty array |
| Equal bodies / union | One body | Null |
| Equal bodies / intersection | One body | Null |

Empty arrays and null responses are retained separately. They are observations
of these public SDK calls, not evidence for interactive command behavior.
Command adapters will need their own native workflow evidence. General Rhino
Boolean parity and a kernel performance comparison remain unproven. See
[capture provenance](convex-boolean-provenance.json).

```sh
cargo test --release -p viboceros-geometry brep::boolean
python3 -m unittest tools.rhino_oracle.test_convex_boolean
```
