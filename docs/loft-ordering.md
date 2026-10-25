# Loft profile ordering

[Loft](loft.md) · [Validation](loft-validation.md)

The command orders private copies; source geometry, document selection order,
attributes and groups remain intact through execution and undo/redo. The ordered
`try_loft_nurbs_curves` API retains its supplied order and directions.

Open nonlinear profiles in distinct parallel planes sort along the first
selected curve's supporting-plane normal. The existing OpenNURBS plane query
provides its sign, including rational and zero-area inflected curves. This sign
need not reverse when an open curve reverses.

Lines, coincident profile planes and other open spatial sections use a chain of
geometric endpoint midpoints. It starts with the first selected section and
extends whichever end is nearest an unused midpoint. Equal distances retain
input priority. This uses geometric midpoints even for rational and cubic lines;
parameter midpoints, control averages and closest-segment distances differ.

After ordering, each open section keeps or reverses its traversal to minimize
the summed distances between corresponding endpoints and the preceding section.
The first ordered section retains its supplied direction. This pass is shared
by both construction backends and runs before optional section rebuilding.

Closed profiles retain the existing oriented-area plane order, consecutive seam
tangent alignment and nearest-knot seam policy. Mixed open/closed sets and
nonparallel or nonplanar closed sets have no new ordering policy.

Five retained Rhino 8.32 captures contain 464 Normal command records (456
distinct recipes, with eight repeated pilot cases): all input
permutations, reversal, axis rotations, unequal spacing, shifted and coincident
planes, mixed directions, tilted planes, rational lines and distance ties.
The raw comparisons check coefficients, weights, knots, domains, topology, face
sense, source state and 289 points per face at absolute `1e-9`. Supporting-plane
normals are independently checked against the native metadata.

The [source and capture record](loft-ordering-validation.json) records measured
errors and the separate Loft Rebuild command comparison. This finite corpus
does not qualify arbitrary spatial matching, tolerance-boundary decisions,
extreme model scales, continuous error bounds or complete Rhino parity.

```sh
cargo test -p viboceros-oracle --test loft_ordering_native
cargo test -p viboceros-oracle --features native-smlib --test loft_ordering_native
python3 -m unittest tools.rhino_oracle.test_loft_ordering
```
