# Native NURBS fillet contacts

Linux `native-smlib` builds now use `SmCurveFilletExecutive` to solve curved
NURBS-to-line, NURBS-to-arc and NURBS-to-NURBS contacts in `Fillet` and
`FilletCorners`. The shared geometry walker retains picking, corner ordering,
analytic line/arc handling, internal sharp-knot preparation and joining policy.
`CurveFilletOperations` supplies contact and extraction producers;
`CurveFilletPairOperations` also supplies the existing connection producers.
Portable APIs retain their signatures and shared solver.

Each contact solve works around the selected junction, with coordinates divided
by the requested radius and each curve's parameter domain mapped to `[0,1]`.
The native radius is one. This avoids the executive's small-radius arc
construction failure and removes dependence on the two source charts. Common
signed weight gauges normalize without changing the curve. Unequal weights
use the canonical import range `[0.001,100]`. Degree above 17, mixed signs,
full-order interior breaks and unrepresentable working frames or knot charts
retain the shared contact solver; exact extraction uses its existing backend.
Native solve failures remain errors.

The bounded [source](../third_party/patches/usd-brep-curve-fillet-seed.patch)
and [header](../third_party/patches/usd-brep-curve-fillet-seed-header.patch)
patches add explicit base-curve guesses, an intersection tolerance and
unsnapped contact parameters. Known selected ends seed the local offset
intersection directly, so an earlier crossing cannot choose another corner.
Existing entry points retain their global intersection search. Evaluation
and arc-construction statuses now propagate. No class layout changes.
Both CMake builds generate patched copies with context fuzz disabled;
Cargo tracks all fillet patches. The upstream submodule stays unchanged.

The C bridge runs under the existing kernel lock. The executive receives
independently imported working curves with trimming and joining disabled.
Rust restores the native circular arc and original contact charts. Analytic
arc parameters come from their contact points: an arc's angle chart differs
from its exact rational-quadratic parameterization.

The geometry core independently checks radius, source contact positions,
directions and tangent angles before extraction, then checks the actual
retained geometry again. Invalid producer output cannot authorize an edit.
The precision regression at origin `(100,-100)` and feature scale `1e-6`
shows why the second check matters: original contacts are accurate, but
rounded retained control points lose the requested angular tolerance. Both
backends reject that request. Unit-conversion tests scale the dimensional
tolerance with the geometry; acceptance tolerances are unchanged.

The existing [contact matrix](../crates/viboceros-smlib/tests/fillet_contacts.rs)
now exercises the native solver, including shifted charts, signed gauges,
reversal, closed seams, internal kinks and degree-18 fallback. New cases check
the selected corner in a pair with an earlier crossing, three model scales,
translated frames and rejection of unrepresentable retained tangency.
Injected producer errors and invalid parameters, loci or directions also
have explicit regressions.

The [C ABI qualification](smlib-native-fillet-contacts-producer-comparison.json)
retains all 36 line/arc/NURBS/earlier-crossing cases across three charts and
three planes, plus invalid-input rejection. AddressSanitizer instruments
SMLib, SM_API and the C bridge; only global context leak accounting is disabled.
The [SDK replay](smlib-native-fillet-contact-comparison.json) uses the same
55 saved Rhino fixtures and their existing per-case ceilings. The earlier
extraction-only records identify their original sources at `2cf12526`.
All 243 native-suite tests, 30 geometry fillet regressions, targeted command
and oracle regressions, and native/portable Clippy checks pass.
Fresh Rhino capture remains blocked by the
[recorded Wine startup failure](smlib-extension-rhino-capture.json).

The [backend harness benchmark](smlib-native-fillet-contact-benchmark.json)
compares the same 51 fillet fixtures with source construction and output
sampling/export outside the timed loop. It compares portable and native
Viboceros work; it does not measure a speedup against Rhino. All 51 output
comparisons pass. At 100 iterations, the median portable/native timing ratio
for NURBS-labelled fixtures is `6.54`. General branch
qualification, arbitrary conditioning, rational smooth Connect parity and
whole-model performance remain further work.

```sh
cargo test --release -p viboceros-smlib --features native --test fillet_contacts
cargo test --release -p viboceros-geometry --lib fillet
python3 -m tools.numerics.compare_native_curve_connections /tmp/contacts.json \
  --include-corners
python3 -m tools.numerics.benchmark_curve_fillet_contacts /tmp/timings.json
```

Build `viboceros_usd_brep_fillet_contacts_probe` with both
`VIBOCEROS_CURVE_FILLET_SIDE_PATCH` and `VIBOCEROS_CURVE_FILLET_SEED_PATCH`
enabled. For the sanitizer build, set `CMAKE_CXX_FLAGS` to
`-fsanitize=address -fno-omit-frame-pointer`, then run:

```sh
python3 tools/usd_brep/compare_native_fillet_contacts.py /tmp/producer.json \
  --probe /tmp/contacts/viboceros_usd_brep_fillet_contacts_probe \
  --asan /tmp/contacts-asan/viboceros_usd_brep_fillet_contacts_probe
```
