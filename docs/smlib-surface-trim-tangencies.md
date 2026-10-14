# Fitted surface intersections near trim tangencies

The native general surface/face adapter now retains connected curve spans near
trim tangencies when it can prove their complete model-space correspondence to
an original boundary image. This prevents a small fitted-UV excursion through a
hole from falsely splitting the intersection into separate objects.

The retained baseline uses `z=x²+y³` against `z=1−xy`, with a rational circular
hole tangent to `x²+y³+xy=1` at `x=0.45`. At model tolerance `1e-6`, the prior
adapter emitted two curves. The circle center is offset by its radius along the
analytic normal; the independent intersection stays outside the hole and should
remain one connected curve. The comparison preserves that failure log and the
Git-backed source hashes of the original implementation.

Each trim still uses the native curve/curve intersector in normalized UV space.
Between consecutive contacts on the same original trim, a midpoint classified
outside the face can trigger an additional boundary-image proof. The spatial
restriction and directed original UV trim restriction enter the existing exact
rational/outward-interval `parameter_curve_deviation_bound` certificate. Only
`Some(bound)` within the effective model-space limit admits the whole range.
Near endpoints or a small parameter interval alone do not authorize admission.
A closed trim crossing its stored seam uses two directed original UV pieces and
two separate spatial restrictions, with matched-parameter proofs on both.

Interval classification includes these certified boundary ranges for each
operand separately. Consecutive retained ranges are merged as parameter ranges
on the **same original curve**. The adapter reconstructs one XYZ restriction
and both original UV restrictions, then recertifies the result on both original
surfaces. It does not move endpoints, join different fitted curves or introduce
a fitting solver. Actual crossings deeper than the model-space limit remain
excluded. Trim queries and optional boundary proofs share the one-million-work
budget; individual certificates keep their established degree/arithmetic limits.

Rust regressions cover 12 analytic tangent-circle configurations: three contact
positions, both normal offsets, and model tolerances `1e-6` and `1e-9`. Four
additional configurations check penetration and clearance at 64 times the
model tolerance. Existing rectangular/circular holes, both face restrictions,
corner points, signed gauges and extreme charts remain covered. Acceptance tests
check both seam directions, invalid boundary parameters, and an interior bow
whose endpoints coincide with the boundary but whose complete span does not.
Complete closed-boundary images are checked separately from short seam arcs.
Qualification passes 274 native adapter tests, 92 native intersection command
tests, 24 cutting Split tests, ten acceptance-hook tests and native/portable lint
with warnings denied.

[Retained qualification](smlib-surface-trim-tangencies-comparison.json) runs
24 actual C ABI tangent-circle queries in normal and complete AddressSanitizer
builds: both source UV witnesses, independent source charts, three spatial planes
and both normal offsets. Native results contain zero or two contacts depending
on the fitted displacement. Those counts diagnose the native producer and do
not prove complete discovery. Each copied contact is checked against the
original UV and model-space images; source buffers and owned snapshot lifetimes
are checked. Leak detection is disabled for the retained process-global context.

The comparison also replays all 121 saved intersection/cutting command records
at absolute `1e-8`, relative `1e-12`, keeping complete native/portable responses
and SDK comparisons. Tangency regressions are independent analytic tests, not
fresh Rhino observations. The boundary proof establishes model-space closeness,
not general region topology, injectivity or complete contact discovery. Cases
without suitable native contact pairs can retain the existing classification.
Arbitrary conditioning and speed relative to Rhino remain unqualified.

```sh
cargo test -p viboceros-smlib --features native --test surface_intersections
cargo test -p viboceros-geometry --lib surface_surface_backend
cmake -S tools/usd_brep -B /tmp/trim-tangencies -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DVIBOCEROS_CURVE_INTERSECTION_DISTANCE_PATCH=ON \
  -DVIBOCEROS_CURVE_SELF_INTERSECTION_PAIRS_PATCH=ON
cmake --build /tmp/trim-tangencies --target viboceros_usd_brep_surface_tangent_adapter_probe
/tmp/trim-tangencies/viboceros_usd_brep_surface_tangent_adapter_probe
```

Use `RelWithDebInfo`, compiler flags `-fsanitize=address -fno-omit-frame-pointer`
and linker flags `-fsanitize=address` for ASan. See
`python3 -m tools.usd_brep.qualify_surface_trim_tangencies --help` for the complete
comparison inputs, including the retained baseline log.
