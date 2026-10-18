# Unit directions and model tolerances

Valid circles, caps and intersections failed when the document's absolute
distance tolerance reached or exceeded one. Internal frame construction was testing
already normalized directions against that distance. The same error appeared
in projected circle axes and the unit-axis permutation used by MeshEllipsoid.
Increasing geometry and its distance tolerance together therefore changed
which directions the kernel considered valid.

`Frame3::try_from_unit_normal(origin, normal)` now accepts a `UnitVector3`,
retains that exact normal and shares the deterministic normal-frame algorithm.
It requires no model-distance tolerance. The existing raw-vector constructor
retains its magnitude admission policy for callers supplying physical vectors.
Known unit normals now remain typed through planar-face projection, capping,
mixed planar Booleans, sphere intersection and crossed-torus section frames.

Circle projections and crosses of unit directions use the existing frame-style
angular threshold. Model-distance checks remain on radii, chords and radial
point displacements. Parallel and angularly unresolved directions still fail;
changing the model distance tolerance cannot authorize those directions.
MeshEllipsoid's internal permutation reconstructs its already validated unit
axes with numerical validation, while retaining the mesh's geometric validation.

The regression uses scales `2^-20`, `1`, `2^20`, `2^40`, `2^80`. Its translated,
tilted frames exercise distance tolerances from roughly `9.54e-14` to `1.21e17`.

| Operation | Cases | Before: successful | After: successful |
| --- | ---: | ---: | ---: |
| Five circle construction modes | 25 | 15 | 25 |
| Each missing face of a box, capped | 30 | 18 | 30 |
| Sphere/sphere intersection | 5 | 3 | 5 |
| Crossed equal torus intersection | 5 | 3 | 5 |

Circle samples retain radius and plane. Caps preserve the exact source
vertices and edges, close the solid and retain normalized volume `30` within
`1e-8`. Sphere intersections retain two analytic circle pieces with normalized
sphere-radius errors below `1e-12`. Torus intersections retain four closed
sections with normalized torus residuals below `5e-7`. Sources remain unchanged.

The earlier planar Boolean matrix now covers 120 cases through `2^80`, retaining
the same analytic formulas, component counts and normalized area bound `1e-6`.
The measured maximum area error remains about `4.98e-14`. Additional checks
retain the supplied unit normal exactly, test handedness and orthogonality,
preserve raw-vector magnitude rejection, and check angular admission at six
different model-distance tolerances.

Command regressions cover 20 Circle/Sphere/Ellipsoid/MeshEllipsoid recipes plus
five IPlane intersections and five Arc Extension endpoint frames. They check
analytic bounds or mesh-vertex ellipsoid equations, closed mesh topology,
unchanged sources and exact Undo/Redo geometry. The affected command adapters
now reuse validated unit directions rather than validating their length again.

The full application baseline also exposed a stale autocomplete test. Its
case-insensitive prefix now correctly expects `ExtrudeCrvTapered` followed by
`ExtrudeCrvToPoint`; completion behavior is unchanged.

See the [validation record](unit-direction-validation.json) for retained
baseline/candidate logs, exact source hashes and completed suite results.
This is analytic and retained-capture regression evidence, with no new Rhino
capture or performance claim. Other raw-direction adapters, including axial
primitive direction options and some construction-plane/orientation helpers,
still require a separate dimensional-policy audit. Arbitrary scales, angular
degeneracies and general modeling topology are not established by these cases.

```sh
cargo test -p viboceros-geometry --test unit_direction_scale --test planar_boolean_scale
cargo test -p viboceros-command --test unit_direction_scale
cargo test --release -p viboceros-geometry -p viboceros-command --features viboceros-command/native-smlib
cargo test --release --bin viboceros
```
