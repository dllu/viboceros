# Native NURBS fillet extraction

[Native contact solving](smlib-native-fillet-contacts.md) builds on this
extraction adapter. The records below retain the earlier implementation at
commit `2cf12526` through immutable Git references.

Linux `native-smlib` builds use the checked usd-brep segment producer for
NURBS contact intervals in `Fillet` and `FilletCorners`. The shared contact
solver retains its root selection, circular arcs and analytic leaf edits.
`PolyCurve3::try_fillet_corners_with_nurbs` delegates NURBS extraction for
candidate removed-length measurement, final retained curves and sharp internal
knot preparation. Pair filleting passes its existing native operations through
the same method. The command and timed oracle choose the same backend.
Geometry errors retain their command type, source definitions stay unchanged
through preparation, and degree-18 or higher extraction stays portable.

Small NURBS-to-NURBS corners now refine center intersections to satisfy both
distance and angular criteria. The numerical floor uses local projected
coordinates and radius; an artificial unit scale previously stopped small
corners too early. Independent symmetric quadratic contacts check this at
scales `1` and `1e-6` in ordinary and shifted domains. Acceptance tolerances
are unchanged. Smooth unselected straight runs also retain their original
line, polyline and NURBS classes and complete local parameter domains.

The [native regression](../crates/viboceros-smlib/tests/fillet_contacts.rs)
covers 61 jobs: NURBS-to-line/arc/NURBS contacts, reversed curves, shifted
charts, common weights `1`, `1e-280`, `-1e280`, small corners, exact unselected
leaves, closed seams, sharp internal knots and degree-18 fallback. Complete
definitions and 33-point grids are compared with portable output, alongside
radius and tangent checks. Coordinates must agree within `1e-9` and weight
ratios within `1e-10`. In successive corners, small differences in an earlier
native extraction can shift a later contact parameter by an ULP; knot
comparisons allow 16 machine epsilons times their chart scale. Unselected
definitions remain exact. Injected failures cover candidate and retained
intervals and sharp-knot preparation.

The [expanded SDK replay](smlib-fillet-contact-comparison.json) includes the
31 saved FilletCorners cases, 20 pair-filleting cases and four scripted
Connect cases. Each retains its existing ceiling: nine NURBS corner cases
use `1e-7`, eleven analytic arc corner cases use `1e-8`, and the other corner
cases use `1e-10`. Pair fillets use their earlier `1e-7` NURBS and `1e-10`
analytic ceilings. Fresh Rhino capture is still blocked by the
[recorded Wine startup failure](smlib-extension-rhino-capture.json).
All 55 comparisons pass; the largest coordinate difference is `5.05e-8`.
All 239 native-suite tests, 29 shared geometry fillet tests, targeted command
and oracle regressions, and native/portable Clippy checks pass.

The [direct native executive probe](../tools/usd_brep/curve_fillet_probe.cpp)
found a separate `SmCurveFilletExecutive` defect: it selected the second
curve's side using the first curve's parameter. The
[bounded patch](../third_party/patches/usd-brep-curve-fillet-sides.patch) uses
the second parameter. CMake compiles a generated source copy and stops if the
patch fails; the pinned upstream tree stays unchanged.

The [retained producer comparison](smlib-curve-fillet-producer-comparison.json)
checks independent 80-digit parabola contacts at three scales and three
parameter charts. The repair restores the shifted-domain failures at scales
`1` and `1e6`: six corrected cases pass. All three `1e-6` cases still return
`SM_ERR_NULL_POINTER`; their full jobs are retained. AddressSanitizer checks
the complete SMLib and SM_API on all nine corrected jobs, with only global
context leak accounting disabled. The later normalized native contact adapter
addresses these scale failures with explicit seeds and checked contacts.
Rational smooth Connect parity and whole-model performance also remain further
work.

```sh
cargo test --release -p viboceros-smlib --features native --test fillet_contacts
cargo test --release -p viboceros-geometry --lib fillet
cargo test --release -p viboceros-command -p viboceros-oracle \
  --features viboceros-command/native-smlib,viboceros-oracle/native-smlib fillet
python3 -m tools.numerics.compare_native_curve_connections /tmp/fillet-contacts.json \
  --include-corners
```

Build the direct probe with `VIBOCEROS_CURVE_FILLET_SIDE_PATCH=OFF`/`ON` in
separate build directories. Add `-fsanitize=address -fno-omit-frame-pointer`
to `CMAKE_CXX_FLAGS` for the sanitizer build, then retain all outcomes with:

```sh
python3 tools/usd_brep/compare_curve_fillet_producer.py /tmp/producer.json \
  --baseline /tmp/fillet-before/viboceros_usd_brep_curve_fillet_probe \
  --corrected /tmp/fillet-after/viboceros_usd_brep_curve_fillet_probe \
  --asan /tmp/fillet-asan/viboceros_usd_brep_curve_fillet_probe
```
