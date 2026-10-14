# Native curve intersections

Linux `native-smlib` builds use usd-brep's `SmCurve::GlobalCurveIntersect` for
point and overlap proposals in Intersect, curve-cutting Split/Trim and NURBS
Offset/OffsetMultiple joins and region checks. Document selection, attributes,
output types, source replacement, history and intersection choice remain in the
existing command/geometry policies. Portable builds retain their existing search.

The adapter uses the shared canonical working-curve preparation: translated and
scaled geometry, unit parameter domains and common signed weight gauges. It
restores paired parameters to the original domains. Degree above 17, mixed-sign
or excessive weight ratios, full-order interior breaks and unrepresentable
frames retain the portable producer. Supported native failures propagate.

`NurbsCurve::intersection_events_with_curve_with_operation` validates source
bounds before invoking the producer and independently evaluates every paired
contact on both original curves. The effective absolute/relative distance policy
is shared with the portable search. Overlap interiors receive endpoint and 15
interior checks per source span in both directions against the other restricted
curve. They do not assume paired parameters vary linearly: reversed and nonlinear
parameter mappings remain valid. The common finalizer deduplicates contacts,
merges adjacent ranges, removes contacts covered by overlaps and orders events.
The 500,000-record/check budgets bound accepted producer data and overlap work.

Native point parameters receive existing local Newton/tangency polishing in a
bounded neighborhood of the proposed span pair. This removes rounded contact
families at multiple-knot tangencies and retains exact simple line crossings.
It does not run the portable global intersection search for supported inputs.
A circle touched once remains unchanged by cutting Split; Intersect still
preserves pairwise duplicate output objects.

The [generated source repairs](../third_party/patches/usd-brep-curve-intersection-distance.cmake)
address three independently observed upstream limits:

- Near tangency, small normal-equation residuals could terminate before physical
  coincidence, discarding two real crossings on `C(t)=(2t,4t(1-t),0)` and
  `Y=1-1e-10` at tolerance `1e-12`. An optional spatial-convergence mode checks
  the evaluator distance at acceptance/early-stop points. Curve intersection
  enables it; other solver callers retain their default policy. A separate
  tangent-objective retry handles shallow true tangencies without switching
  objectives while resolving nearby transverse roots.
- Generic parameter deduplication uses `1e-4` of the domain and could merge the
  two crossings even after solving them. Curve/curve point comparison now also
  checks physical separation with an intersection-angle uncertainty estimate.
  Projected operations and other global solver types retain their previous path.
- Endpoint and midpoint guesses could all fail on a shallow off-center contact.
  Four interior quarter-pair guesses reuse the same native solver; arrays hold
  all nine initial and up to five existing additional guesses.

The pinned submodule is unchanged; CMake applies patches with fuzz disabled.
The patched header is included before upstream headers, and a public build
configuration definition forces consistent recompilation when enabling this
class-layout extension. This prevents reuse of cached objects built against the
original solver layout.

[Retained qualification](smlib-curve-intersections-comparison.json) contains
normal and complete AddressSanitizer C ABI jobs, each with 36 analytic cases:
transverse/tangent/two-root contacts, forward/reversed overlaps and disjoint
curves, two independent parameter charts and three planes. Analytic paired
images, expected parameters, unchanged source evaluations, ownership and invalid
input are checked. ASan covers SMLib, SM_API and the bridge; process-global
context leak accounting alone is disabled.

The same record retains original and patched direct diagnostics for 12 inputs
in both coincidence modes, including near-disjoint geometry, shallow tangencies
and a one-ULP normalization variant. Rust regressions additionally cover 108
chart/plane/gauge combinations, rational contacts, nonlinear reversed overlaps,
multiple-knot and off-center tangencies, short spans, unclamped linear precision
and unsupported-basis fallback. Shared callback tests reject incorrect contacts,
domains and overlap interiors and preserve producer errors.

Command comparisons and saved Rhino replays are retained separately. The saved
Split records exercise curve cutting; trim-geometry records exercise extraction.
These are prior SDK observations, not fresh captures. Wine licensing startup
still blocks fresh instrumentation as described in [isocurve qualification](smlib-isocurves.md).

The unchanged whole-domain trim outer-knot difference is retained as a raw failure:
Viboceros stores `-2`/`4`, while Rhino pads the two omitted full-vector slots with
`-1`/`3`. The comparison identifies it from the immutable earlier
[trim qualification](smlib-curve-segments-comparison.json); it does not rewrite
either result or relax the epsilon. The active locus and stored basis agree.

Neither source-space sampling nor native numerical search certifies continuous
coincidence or complete event discovery. Arbitrary conditioning, very high root
counts and performance relative to Rhino remain unqualified.

The complete native suite passes 254 tests. Focused command checks pass eight
curve-cut, 86 intersection and 36 curve-offset tests. Native/portable lint with
warnings denied and formatting pass.

```sh
cargo test --release -p viboceros-smlib --features native
cargo test -p viboceros-command --features native-smlib curve_cut
cargo test -p viboceros-command --features native-smlib intersect
cargo test -p viboceros-command --features native-smlib offset_curve
cmake -S tools/usd_brep -B /tmp/intersections \
  -G Ninja -DCMAKE_BUILD_TYPE=Release \
  -DVIBOCEROS_CURVE_INTERSECTION_DISTANCE_PATCH=ON
cmake --build /tmp/intersections --target \
  viboceros_usd_brep_curve_intersect_probe viboceros_usd_brep_intersection_adapter_probe
```

For the original diagnostic use the same configuration with the distance patch
OFF. For ASan, use `RelWithDebInfo`, compiler flags
`-fsanitize=address -fno-omit-frame-pointer` and executable/shared linker flags
`-fsanitize=address`. Retain the direct diagnostic JSON from each configuration,
then run `python3 -m tools.usd_brep.qualify_curve_intersections --help` for the
comparison inputs.
