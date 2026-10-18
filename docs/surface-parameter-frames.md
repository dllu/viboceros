# Surface frames and parameter units

Changing a regular surface's UV units could break its tangent frame,
`ArraySrf`, or rigid/nonrigid `OrientOnSrf` without changing its geometry.
Derivative lengths were being compared with model distance tolerance. Splop
construction also multiplied derivative speeds, and its normal evaluation
crossed raw derivatives, allowing intermediate overflow or underflow.

Surface frames now normalize each derivative independently and apply the
existing angular admission policy. Splop uses unit-angle ratios and the existing
scaled parameter-step helper to invert its differential. For derivative lengths
`U`, `V` and unit-direction cosine/sine `c`, `s`, the triangular inverse is
`[1/U, -c/(U*s); 0, 1/(V*s)]`. Evaluation avoids multiplying `U` and `V`.
Mapped normals and constrained-orientation frames use the same typed directions.
Zero derivatives and angularly unresolved tangent pairs still fail.

`ArraySrf` parses Up/Normal as a unit direction. OrientOnSrf's supplied source
normal is normalized independently of its magnitude, and its rotated target
axes use angular frame validation. Its existing numerical source-reference
policy is retained: a nonzero normal projection below the document's angular
tolerance may still define the source plane. Coincident references, parallel
source normals and tangent constrained destination normals remain invalid.

Tests use translated oblique geometry at model scales `2^-20`, `1`, `2^40`,
`2^80`, with absolute tolerance `1e-7` times scale and angular tolerance `1e-9`.
UV domain-width exponent pairs are `(0,0)`, `(-200,-200)`, `(200,200)`,
`(-600,-600)`, `(600,600)`, `(-600,600)` and `(600,-600)`.

| Matrix | Cases | Before successful | After successful |
| --- | ---: | ---: | ---: |
| Surface frames, reversals and swapped UV | 672 | 288 | 672 |
| Analytic splop maps | 168 | 48 | 168 |
| UV/isocurve surface arrays | 336 | 72 | 336 |
| Rigid/nonrigid surface orientation | 672 | 168 | 672 |

The analytic target is `(2u+v, 3v, w*u*v)`, with `w=0` or `1`, transformed
by the known model pose. Tests compare origins, axes and point images against
its closed-form derivatives and normals. Frame-origin normalized error is at
most `2e-12`; splop point error is at most `2e-11`. Arrays require point error
below `2e-11`; OrientOnSrf allows `2e-6`, including its closest-point solve.
Source objects and surfaces remain unchanged. Direction magnitudes `2^-200`,
`1`, `2^200` must produce exactly equal command geometry, with exact object
identity and geometry retained through Undo/Redo.

Further checks cover 174 angular/zero-derivative admission cases across six
absolute tolerances from `1e-24` to `1e100`, 168 constrained-normal cases,
and 24 command admission cases. Invalid commands retain objects, selection
and the existing redo history. These bring the focused matrices to 2,214 cases.

The [validation record](surface-parameter-validation.json) retains baseline test
sources, complete logs and source hashes. Existing native observations remain
part of the full suite; these matrices add analytic evidence, with no new Rhino
capture or performance measurement. They do not establish arbitrary parameter
or model scales, general surface singularity handling, all morph consumers,
or general topology. Other derivative consumers, including surface pullback,
remain outside this correction.

```sh
cargo test -p viboceros-geometry --test surface_parameter_frames
cargo test -p viboceros-command --test surface_parameter_placement
cargo test --release -p viboceros-geometry -p viboceros-command --features viboceros-command/native-smlib
cargo test --release --bin viboceros
```
