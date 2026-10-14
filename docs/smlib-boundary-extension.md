# Native boundary-target Extend

Linux `native-smlib` builds use the shared native extension adapter when
`Extend` stops at selected curves, surfaces or trimmed B-reps. Nearest-hit
ordering, endpoint picking, natural line/arc classification and the three
joining policies remain shared geometry policy. A dedicated
[boundary module](../crates/viboceros-geometry/src/nurbs/boundary_extension.rs)
delegates extrapolation, interval extraction and selected-endpoint clamping
through `CurveExtensionOperations`. The native implementation reuses
`N_CrvExtendByParamDist`, checked segment extraction and existing endpoint
splitting. Command execution and oracle timings select the same backend.

Joined source preparation preserves the complete unselected knot padding.
It uses native refinement directly, since the public `InsertKnot` edit applies
the separate OpenNURBS outer-knot convention. Source objects and boundaries
remain unchanged until prepared output is committed through the command
transaction. Geometry errors retain their existing command type. Requested
source degrees above 17 retain the portable fallback.

Candidate intersections are independently checked by evaluating both loci at
the reported parameters using the document's coordinatewise tolerance. B-rep
face candidates must also lie inside their trims. Endpoint parameters cannot
authorize a positive-length extension, even if a very distant probe changes
its evaluated endpoint slightly. A large probe's bounding-box scale therefore
cannot turn a displaced point into a valid boundary hit.

Long growth exposed a native status defect: `N_CrvRemoveKnot` substitutes a
conservative basis bound of one after failed extremum searches but returns the
handled error. The [bounded patch](../third_party/patches/usd-brep-knot-removal-bound-error.patch)
clears that status in three existing fallback branches. It changes neither
the removal tolerance nor the acceptance test. CMake applies the patch to a
generated source copy and stops on patch failure; the pinned submodule remains
unchanged. Large curves can retain endpoint knots when the existing numerical
removal test rejects their removal.
The low-level last-error diagnostic can still record that handled search
failure; the producer's returned status and geometry checks determine success.

The adapter grows by bounded portions of the current end span, balancing the
last two stages to avoid a tiny remainder. Each stage calls the existing native
producer. Native storage is recycled between stages so one arena does not
retain every complete intermediate curve. Equal source weights describe a
polynomial: those inputs use an exact unit denominator during native work and
recover their original common signed weight on export. This prevents small
weight-rounding errors from becoming a growing rational denominator.

The [Rust regression](../crates/viboceros-smlib/tests/boundary_extension.rs)
checks 324 combinations of clamped/unclamped polynomial sources, common gauges
`1`, `1e-280`, `-1e280`, curve/surface/B-rep boundaries, Start/End/Both,
Smooth/Natural and Merge/Yes/No. Farther boundaries are deliberately ordered
first. Complete definitions and position grids are compared with the portable
producer, while `C(t)=(4t,6t(1-t),0)` independently checks every output.
Rational quotient checks cover all three joining modes and gauges. Full-order
breaks, degree-18 fallback, missing/closed requests and injected producer
failures are also covered.
Both-end requests are checked when only one side has a reachable boundary;
the valid side must still succeed without a false hit on the missing side.

The [direct probe](../tools/usd_brep/extension_growth_probe.cpp) checks 16
polynomial extensions through distances `2^39`. The
[real adapter probe](../tools/usd_brep/extension_adapter_probe.cpp) checks 30
growth jobs through the C ABI, including all three gauges. Original-domain
positions must agree within `1e-9`; distant positions use `1e-12` relative
error, and common weights and requested domains are exact. The
[retained producer comparison](smlib-boundary-extension-producer-comparison.json)
includes every baseline, corrected and sanitizer outcome and source hashes.
AddressSanitizer instruments the complete kernel and native bridge; only
process-wide context leak accounting is disabled.

The Python extension comparison accepts `--include-boundaries`, adding the
22 existing boundary fixtures to the earlier 60 extension cases. Those include
arc/line policies, surface tangency, coplanar trimmed-face entry and skipped
holes. The [retained backend comparison](smlib-boundary-extension-comparison.json)
uses actual native and portable engine identities. Fresh Rhino validation
remains unavailable due to the
[recorded Wine licensing startup error](smlib-extension-rhino-capture.json).
The 82-case record identifies its original sources at commit `38592627`
through immutable Git references.
The earlier [60-case record](smlib-extension-comparison.json) now identifies its
original source through immutable Git references.

All 82 backend comparisons pass; the maximum numeric difference is `2.84e-14`
overall and `8.89e-16` in the 22 boundary cases. The full native suite, 30 command
and 77 geometry boundary tests, 1,100 Python tests, and native/portable Clippy
checks pass. A broader oracle `boundary` filter also runs an unrelated Join
replay that compares native stored edge tolerance `1e-9` with Rhino's `0`.
That same test and field fail on the preceding `1a4e8079` commit in an isolated
worktree; the [baseline comparison](smlib-boundary-extension-join-baseline.json)
retains both outcomes. The expected observations and that Join test are unchanged.

```sh
cargo test --release -p viboceros-smlib --features native --test boundary_extension
cargo test --release -p viboceros-command -p viboceros-oracle -p viboceros-geometry \
  --features viboceros-command/native-smlib,viboceros-oracle/native-smlib boundary
python3 -m tools.numerics.compare_native_extensions /tmp/boundary-extensions.json \
  --portable-baseline --include-boundaries
```

Build standalone probes in separate directories with
`VIBOCEROS_EXTENSION_BREAK_PATCH=ON` and
`VIBOCEROS_KNOT_REMOVAL_BOUND_PATCH=OFF`/`ON` for the baseline and corrected
kernel. Build the sanitizer variant with
`-DCMAKE_CXX_FLAGS="-fsanitize=address -fno-omit-frame-pointer"`.

```sh
cmake --build /tmp/growth-after --target \
  viboceros_usd_brep_extension_growth_probe viboceros_usd_brep_extension_adapter_probe -j 10
python3 tools/usd_brep/compare_extension_growth.py /tmp/growth.json \
  --before /tmp/growth-before/viboceros_usd_brep_extension_growth_probe \
  --after /tmp/growth-after/viboceros_usd_brep_extension_growth_probe \
  --adapter /tmp/growth-after/viboceros_usd_brep_extension_adapter_probe \
  --asan /tmp/growth-asan/viboceros_usd_brep_extension_growth_probe \
  --asan-adapter /tmp/growth-asan/viboceros_usd_brep_extension_adapter_probe
```

Arbitrary conditioning, rational poles, whole-model Rhino parity and performance
against native Rhino remain further work. Intersection search itself continues
to use the shared geometry algorithms.
