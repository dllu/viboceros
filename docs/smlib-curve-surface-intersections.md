# Native curve/surface intersections

Linux `native-smlib` builds use usd-brep's `SmSurface::GlobalCurveIntersect` for
curve/surface contacts in Intersect, IntersectTwoSets, cutting Split/Trim and
surface Split with curve cutters. Curve/B-rep contacts use the same producer
for each underlying face, retaining shared UV trim containment, hole clipping,
singular-vertex handling, event merging and source replacement policies. Generic
trim-edge contacts also use the existing native curve/curve producer; exact axis
certificates retain their shared path.

Native queries reuse the B-rep surface importer. Each query owns a temporary
surface, and returns copied curve/U/V parameter records through an independently
owned event buffer. Native objects and source buffer pointers do not enter the
document or history. Calls and destruction use the process-wide kernel lock.
Input and output domains, event kinds and the 500,000-event budget are checked.
Invalid inputs clear the output pointer before returning an error.

Curve and surface preparation share translated/scaled controls and common signed
weight normalization. All three active parameter charts become `[0,1]`, with
explicit checks against collapsed knot intervals. Unclamped surface ends use
existing native endpoint knot insertion before import; no new endpoint control
solver is introduced. Returned parameters map to the original curve and surface
charts. Degree above 17, full-order interior breaks, mixed/excessive weight ratios
and collapsed surface frames retain the portable search. Native failures on
eligible inputs propagate.

`curve_surface_intersection_events_with_operation` independently evaluates every
proposed contact on both original geometries. It uses the existing effective
absolute/relative model-space distance policy. Point proposals reuse bounded
local Newton/tangency polishing. Every overlap span receives 17 original-curve
stations checked against the original surface, without assuming a linear UV
mapping. The common finalizer orders events, merges ranges and removes points
covered by overlaps. These sampled checks do not certify continuous coincidence
or complete event discovery.

`curve_brep_intersection_events_with_operations` keeps face-local trims in their
original UV charts during a common view projection. Dimension-collapsed projected
faces can therefore use the portable query without constructing an invalid
projected B-rep. The original objects, attributes, groups, selection and atomic
history retain the existing command policies.

[Retained qualification](smlib-curve-surface-intersections-comparison.json) runs
54 actual C ABI cases in normal and full AddressSanitizer builds: transverse and
tangent plane contacts, finite interior/partial/boundary overlaps, disjoint curves,
and polynomial/rational bilinear patches, with independent curve/U/V charts in
three planes. Independent parameter/image formulas, unchanged source evaluations
and surface inputs, event copying, invalid degrees/tolerances and null input are
checked. SMLib, SM_API and the bridge are instrumented; process-global context
leak accounting alone is disabled.

Rust regressions cover 162 chart/plane/common-gauge combinations, unclamped active
ends, parameter scales `2^-360` and `2^360`, analytic cylinder tangency, sphere
seams and poles, face holes and view projection. Unsupported degree and collapsed
frames retain the portable results. Callback tests reject incorrect contacts,
invalid domains and false overlap interiors, and check nonlinear UV mappings,
range merging, covered points and producer errors.

The comparison replays all 121 saved intersection/cutting command records at
absolute `1e-8`, relative `1e-12`, retaining complete native/portable responses and
raw SDK comparisons. These are saved observations, not fresh Rhino captures.
All 121 complete records match, with no native execution errors. Qualification
also passes 264 native adapter tests, 89 native intersection command tests, eight
curve-cut regressions, and native/portable lint with warnings denied.
The existing Wine licensing startup failure still blocks fresh instrumentation.
Arbitrary conditioning, general overlap completeness and performance relative to
Rhino remain unqualified.

```sh
cargo test -p viboceros-smlib --features native --test curve_surface_intersections
cargo test -p viboceros-geometry --lib curve_surface_backend
cargo test -p viboceros-command --features native-smlib --lib intersect
cmake -S tools/usd_brep -B /tmp/curve-surfaces -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DVIBOCEROS_CURVE_INTERSECTION_DISTANCE_PATCH=ON \
  -DVIBOCEROS_CURVE_SELF_INTERSECTION_PAIRS_PATCH=ON
cmake --build /tmp/curve-surfaces --target viboceros_usd_brep_curve_surface_intersection_probe
/tmp/curve-surfaces/viboceros_usd_brep_curve_surface_intersection_probe
```

Use `RelWithDebInfo`, compiler flags `-fsanitize=address -fno-omit-frame-pointer`
and linker flags `-fsanitize=address` for ASan. See
`python3 -m tools.usd_brep.qualify_curve_surface_intersections --help` for the
retained comparison inputs.
