# Native surface-blend producer qualification

The standalone [probe](../tools/usd_brep/surface_blend_probe.cpp) constructs two
independent exact planar sheets and invokes `SmBrep::CreateEdgeEdgeBlend`
directly. Four rail-domain pairs cover equal `[0,1]`, equal shifted `[10,30]`,
and unequal `[0,1]`/`[2,5]` and `[10,30]`/`[2,5]`. Each is tested as ruled, G1
and G2, with both explicit correspondence directions, in a fresh process.

The pinned unmodified kernel has two verified defects:

- Unequal-domain G1/G2 calls corrupt the heap: all eight isolated cases abort
  or crash. The first rail alignment computes an offset with division applied
  to only one numerator term. For first domain `[10,30]` and second `[2,5]`,
  it transforms the second rail to `[90,110]` while subsequently requesting
  `[10,30]`. The resulting evaluation leaves its valid parameter interval.
- Explicit end-to-start correspondence is ignored. G1/G2 reverse the second
  rail twice; the ruled branch returns before handling the flag. In the
  successful baseline cases, the boundary correspondence error is two model
  units despite native success status `1000`.

The [bounded source patch](../third_party/patches/usd-brep-blend-rail-parameterization.patch)
corrects the affine expression, retains one reversal and the associated side
orientation, and handles explicit reverse correspondence in the ruled branch.
It changes no model tolerance. Native CMake applies it to a generated source
copy; the upstream submodule is immutable, and patch failure stops the build.

All 24 patched calls succeed and return finite samples on a 41×41 grid.
Independent endpoint correspondence agrees within `2e-16`. Same-direction
ruled, cubic and quintic profiles agree with independently expanded Hermite
polynomials within `1e-15`; their tangent lengths are 0.5, so their profiles
intentionally differ from a linearly parameterized strip. The ruled producer
uses U across the strip, while G1/G2 use V. The complete SMLib and SM_API builds
also pass the 24 cases with AddressSanitizer. Leak accounting is excluded for
the process-wide native context; heap access checks remain enabled.
The [retained artifact](smlib-surface-blend-producer-probe.json) keeps every
baseline, patched and sanitizer outcome, including process failures and stderr.

The native bridge builds the same probe against its patched static libraries.
Its `surface_blend_producer` integration test runs all 24 cases in isolated
processes, so a future native abort is an ordinary failed test.

```sh
cargo test --release -p viboceros-smlib --features native --test surface_blend_producer
cmake -S tools/usd_brep -B /tmp/usd-brep-probe -DCMAKE_BUILD_TYPE=Release \
  -DVIBOCEROS_BLEND_RAIL_PATCH=ON
cmake --build /tmp/usd-brep-probe --target viboceros_usd_brep_surface_blend_probe -j 10
/tmp/usd-brep-probe/viboceros_usd_brep_surface_blend_probe 3 2 1
```

Omit the option, or set it to `OFF`, to reproduce the unmodified standalone
kernel in isolated processes. Argument order is domain case (0–3), blend flag
(0 ruled, 1 G1, 2 G2) and direction (1 start-to-start, 2 start-to-end).

These fixtures qualify the two repairs, not arbitrary curved or rational
boundary fitting, extreme parameter ranges, automatic correspondence, global
fairness or absence of folds. The opposite-direction parallel-sheet case has
a collapsing interior, so endpoint correctness is not a regularity claim.
The application's exact [BlendSrf construction](smlib-surface-blend.md) still
uses native parent preparation and independent Hermite constraints; switching
its whole producer requires further shape and mixed-continuity qualification.
