# Surface image bounds through planar quadrilaterals

[Geometry guides](README.md) · [UV correspondence bounds](surface-correspondence.md)

`NurbsSurface::planar_surface_image_deviation_bound(other, limit)` bounds the
symmetric Hausdorff distance between complete untrimmed surface images.
It permits different UV speeds, reversals, swaps and folds. `Some(bound)`
certifies the entire images within `limit`; `None` means outside or unproved.
Zero is strict. A finite nonnegative limit is required. Inputs remain unchanged.

The method uses exact auxiliary planar tensors, so nearby nonplanar surfaces
can also receive conservative bounds. It does not certify UV correspondence,
trimming, normals, topology or injectivity. The existing parameter bound and
automatic fitting acceptance policy remain unchanged.

## Certificate

1. Require clamped, continuous U/V bases and nonzero common-sign weights.
   The stored corner controls are therefore attained boundary endpoints.
2. Define a plane from three corners and project the fourth corner onto it.
   Require a strictly convex nondegenerate quadrilateral in boundary order.
3. Project all Cartesian controls onto the plane. Project boundary controls
   additionally onto their corresponding supporting edge lines, and pin corners
   to quad vertices. All projections use rational arithmetic on the exact
   stored binary values; no rounded auxiliary surface replaces the source.
4. Bound each control movement outward. Rational basis coefficients form a
   nonnegative partition of unity, so their maximum movement bounds the whole
   original-to-auxiliary parameter displacement in both image directions.
5. Prove auxiliary containment in all four quad half-spaces. Control hulls
   usually suffice. Otherwise extract exact Bernstein patches and recursively
   bisect their signed scalar numerators until nonnegative hulls prove inclusion.
   An attained negative corner or exhausted resources leaves the proof declined.
6. Each continuous auxiliary boundary side lies on its corresponding quad
   segment with fixed endpoints. It is homotopic along that segment to the
   straight edge, even if it backtracks. The boundary thus winds once around
   every interior quad point. Omitting such a point would extend this nonzero
   winding loop over the parameter disk in the punctured plane, a contradiction.
   Containment and coverage prove that the auxiliary image is the whole quad.
7. Match quad vertices under all eight cyclic/reversed orderings. Triangulate
   corresponding diagonals; paired barycentric coordinates bound displacement
   by the greatest paired vertex distance. Add both control-movement radii
   with outward rounding to bound the original complete images.

Degrees through 16 and at most 4096 controls per source are supported. Shared
certificate limits bound rational bit size, work and subdivision depth.
Discontinuous or unclamped tensors decline because the boundary coverage
argument would not apply. Concave and degenerate corner quads also decline.

## JSON and evidence

The local `surface_planar_image_bound` operation accepts `first` and `second`
surface definitions and optional `absolute_tolerance` (default request tolerance).
It returns `certified` and `maximum_deviation_bound` (null when unproved).

Across 90 successful archived public Rhino `Surface.Fit` outputs, this method
certifies all 30 planar original-source/native-output pairs at `1e-9`. For 22
of these pairs the normalized UV bound is unproved. The other 60 pairs remain
unproved by this restricted image method. These comparisons do not revise the
separate automatic Rust fitter/native-output correspondence ledger.

The [validation record](../surface-planar-image-validation.json) preserves the
proof tests, replay logs and [physical FitSrf investigation](../surface-fit-command-investigation.md).
