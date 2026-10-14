# Native blend derivative scaling

The pinned usd-brep surface-boundary sampler changes a takeoff tangent's
length without changing its second and third derivatives. A cross-parameter
speed change by `a` requires derivatives of orders 1, 2 and 3 to scale by
`a`, `a²` and `a³`. Scaling only the first derivative changes normal curvature,
so an otherwise successful G2 blend can miss its parent's curvature.

The [bounded patch](../third_party/patches/usd-brep-blend-derivative-scaling.patch)
applies those powers inside `SmDerivSurfDefinition::GetSampleValues` when the
existing tangent-length option is active. Native CMake compiles a generated
`SmSurface.cpp` copy and stops if the patch fails. The pinned upstream source,
sampling counts and fitting tolerance are unchanged.

The [curved probe](../tools/usd_brep/surface_blend_curved_probe.cpp) constructs
polynomial and rational quadratic Bezier extrusions. Its second parent has
elevated U/V degrees and a different knot basis. Four equal/unequal rail-domain
pairs, ruled/G1/G2 modes, both explicit correspondences and takeoff factors
`0.2`, `0.5`, `1.7` give 144 isolated constructions. Position checks cover a
41×41 grid, including complete endpoint correspondence. Same-direction
profiles are independently expanded cubic/quintic Hermite polynomials.

Raw boundary samples are checked separately from surface skinning. An
independent rational quotient rule supplies `C`, `C'`, `C''`, `C'''` for the
parent extrusion `S(t,y)=(C(t),y)`. Tangent-plane projection gives the takeoff
direction, and the constant cross-parameter speed gives the higher jets.
The test covers third-order sampling; it does not qualify whole G3 surfaces.

The [retained comparison](smlib-blend-derivative-scaling-comparison.json)
includes every pre-repair, patched and AddressSanitizer process result, source
hashes and diagnostic residuals. The baseline includes the earlier
[rail alignment repair](smlib-surface-blend-producer.md).

| Maximum residual | Before scaling repair | After scaling repair |
| --- | ---: | ---: |
| Raw first derivative | 4.2e-12 | 4.2e-12 |
| Raw second derivative | 26.3 | 1.6e-12 |
| Raw third derivative | 155.3 | 1.3e-12 |
| Boundary position | 9.5e-15 | 9.5e-15 |
| G2 cross normal curvature | 7.13 | 3.3e-8 |
| G2 mixed normal curvature | 4.9e-5 | 4.9e-5 |

The complete second fundamental form is measured against the extrusion's
analytic form using the output's own tangent directions. Reverse correspondence
aims the native takeoff toward the other rail, including a longitudinal
component; its expected cross normal curvature is not generally zero.
The reverse mixed-curvature residual remains a skinning approximation
limitation. Position fitting tolerance alone is not a derivative certificate.
The command continues to use its [exact independent boundary constraints](smlib-surface-blend.md).

Reproduce both standalone producers with the same C++ source:

```sh
cmake -S tools/usd_brep -B /tmp/blend-before -DCMAKE_BUILD_TYPE=Release \
  -DVIBOCEROS_BLEND_RAIL_PATCH=ON -DVIBOCEROS_BLEND_DERIVATIVE_PATCH=OFF
cmake --build /tmp/blend-before --target viboceros_usd_brep_surface_blend_curved_probe -j 10
cmake -S tools/usd_brep -B /tmp/blend-after -DCMAKE_BUILD_TYPE=Release \
  -DVIBOCEROS_BLEND_RAIL_PATCH=ON -DVIBOCEROS_BLEND_DERIVATIVE_PATCH=ON
cmake --build /tmp/blend-after --target viboceros_usd_brep_surface_blend_curved_probe -j 10
python3 tools/usd_brep/compare_surface_blend_producer.py \
  /tmp/blend-before/viboceros_usd_brep_surface_blend_curved_probe \
  /tmp/blend-after/viboceros_usd_brep_surface_blend_curved_probe /tmp/blend-results.json
cargo test --release -p viboceros-smlib --features native --test surface_blend_producer
```

For sanitizer capture, build both complete native libraries with
`-DCMAKE_CXX_FLAGS="-fsanitize=address -fno-omit-frame-pointer"`, then pass that
build's probe as `--asan`. The comparison uses
`ASAN_OPTIONS=detect_leaks=0:halt_on_error=1`; only process-wide native context
leak accounting is excluded.

Arbitrary curved trims, automatic correspondence, rotations of the takeoff
jets, extreme conditioning, fold avoidance, mixed command continuity and Rhino
default-shape parity remain outside these fixtures.
