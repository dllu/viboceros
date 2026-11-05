# Microscopic extrusion spans

[Extrusion geometry](extrusions.md) · [Command input](command-line.md)

Rational profiles can contain substantial geometry inside a knot span only one
floating-point step wide. A reversed UV boundary can lose that span's resolution
while mapping a model-space point back to a trim parameter. Previously this
rejected valid extrusions with an apparent interior edge/trim gap.

B-rep validation now falls back to the existing exact isocurve certificate before
rejecting an unresolved sampled correspondence. It bounds the entire oriented
boundary, including both sides of full-order surface breaks. Model and component
tolerances remain unchanged. Nonisoparametric or unproved images retain the
sampled checks, and proof work is bounded to 16 million charged units per attempt.
The shared certificate also continues to serve edge joining and cleanup.

Mesh generation uses lossless start-anchored curve and face parameter charts.
Every knot and UV control must translate exactly; otherwise the existing local
chart is retained. Stored geometry, native archives and public domains stay
unchanged. Normal and curvature channels use the same charts. Corner `uv` and
`uv_origin` must stay together: adding them in floating point can lose a small
station that the local pair preserves.

The point getter now accepts `Solid=Yes|No` and `Output=Surface` at command start.
`Solid`, `BothSides` and `DeleteInput` can also change between direction picks.
Invalid option groups leave pending choices intact; cancellation preserves the
model, selection and pending Redo.

Sixteen private-Xvfb Rhino commands cover XZ/XY profiles, signed heights,
BothSides, and spans near `1e-14` and `1e-16`. Tests compare complete NURBS controls within `1e-12`, exact weights and knots,
face counts, selection and Undo/Redo. They also check
3DM round trips and the headless application. Kernel checks cover 64 combinations
of these inputs, profile reversal and explicit B-rep conversion, retaining the
rational spike's attained mesh height and regular normals. Altered spatial edges
and nonisoparametric trim excursions still fail validation. General trim images,
continuous mesh error and comparative performance remain unqualified.
The [validation record](short-span-extrusions-validation.json) binds the current
implementation, regression tests and immutable native evidence.

```sh
cargo test -p viboceros-geometry --test extrusion
cargo test -p viboceros-oracle --test short_span_extrusions
cargo test -p viboceros-oracle --features native-smlib --lib curve_extrusion_replay_tests
cargo test -p viboceros short_span_extrusions
python3 -m unittest tools.rhino_oracle.test_short_span_extrusions
```
