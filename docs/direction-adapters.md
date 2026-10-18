# Direction adapters and model units

Explicit primitive `Axis=` options and stored frame axes were being checked
against the document's distance tolerance. Valid commands then failed when
geometry and its distance tolerance were scaled together, or when an equivalent
axis vector had a small magnitude. Construction planes also treated surface
derivatives as physical point offsets, making their validity depend on UV units.

Primitive options now parse nonzero directions into `UnitVector3`. Point-defined
Helix/Spiral axes and projected radius picks still pass model-distance checks
before becoming unit directions. Cap placement translates the existing frame
without reconstructing its axes. Signed-height frames and polygon caps retain
typed normals, while polygon edge lengths still require distance admission.

`Frame3::try_from_unit_directions` and `try_from_unit_x_and_normal` apply the
existing angular policy independently of distance. Both retain the supplied X
axis exactly. Cross products construct the projected normal without amplifying
cancellation in a nearly parallel dot-product subtraction. Raw-vector frame
constructors retain their previous distance checks.

Construction-plane object/tangent frames, rotations, and OrientOnCrv placements
now keep their validated directions. Surface derivatives are normalized without
a model-distance minimum; their angle still determines whether the frame is
resolved. Picked construction-plane X points, rotation axes and reference points
retain distance admission. The native mesh-face normal's f32 storage convention
and the native curve-frame selection rules remain in use.

The regression uses model scales `2^-20`, `1`, `2^20`, `2^40`, `2^80`, with
absolute tolerance `1e-7` times scale and angular tolerance `1e-9`. Explicit axes
are `[2,3,6]` times `2^-200`, `1`, or `2^200`.

| Matrix | Cases | Before successful | After successful |
| --- | ---: | ---: | ---: |
| Twelve primitive recipes, numeric/picked radius | 360 | 164 | 360 |
| Construction-plane operations and UV domains | 65 | 41 | 65 |

The primitive recipes cover Cylinder, Cone, TruncatedCone, Tube, Torus,
MeshCylinder, MeshCone, MeshTruncatedCone, MeshSphere, MeshTorus, Pyramid and
TruncatedPyramid. Tests check analytic surface/vertex equations, closed mesh
connectivity, or normalized solid volumes. Changing only the explicit axis
magnitude must produce exactly equal geometry, retained through Undo/Redo.

Construction-plane checks cover vertical, circle/line object, perpendicular,
mesh face and two rotation modes. Surface object/default frames are checked
under UV domain widths `2^-200`, `1`, `2^200`, with unchanged source surfaces.
Additional tests cover:

- 70 expected rejections for zero directions and short physical axes/picks;
  failed document commands preserve objects, selection and Undo/Redo history.
- 25 Helix/Spiral cases, checking analytic interpolation stations within `1e-12`
  after normalization, radial bounds between stations, and exact Undo/Redo.
  Helices retain their existing cubic approximation, with sampled normalized
  radial error bounded by `1e-6` between stations.
- 45 line/circle/spatial-curve placement cases, checking tangent alignment and
  rigid distances, including perpendicular placement, flips and rotation.
- Three kernel tests checking exact X/origin retention, orthogonality,
  handedness, near-parallel angular admission and raw-vector distance rejection
  across six absolute tolerances from `1e-24` to `1e100`.

The [validation record](direction-adapters-validation.json) retains the original
baseline test source, baseline failure log, final logs and exact source hashes.
The baseline source contains the first two matrices before adding the other
checks. This extends the earlier [unit-normal correction](unit-directions-and-model-tolerance.md).
It adds no new licensed Rhino capture or performance measurement. Arbitrary
scales, unresolved angular directions, all command adapters and general topology
are not qualified by this matrix.

```sh
cargo test -p viboceros-command --test direction_adapters
cargo test -p viboceros-geometry --test direction_frames
cargo test --release -p viboceros-geometry -p viboceros-command --features viboceros-command/native-smlib
cargo test --release --bin viboceros
```
