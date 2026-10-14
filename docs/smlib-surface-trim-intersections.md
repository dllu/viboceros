# Native surface intersections with face trims

Linux `native-smlib` builds now intersect general surfaces with partial B-rep
faces, including holes and restrictions on both operands. Intersect,
IntersectTwoSets and surface cutter queries use this path when the established
analytic dispatcher has no general nonplanar implementation. Known analytic
definitions and coincidence restrictions keep their existing path.

The surface producer retains its spatial curve and both UV witnesses through
acceptance. Each complete proposal first receives the existing continuous
correspondence certificate on both original surfaces. Face loops and their
witnesses then enter a common unit UV frame, using source-axis spans rather
than model-space epsilon to scale coordinates. Native curve/curve contacts
provide breakpoints on original trim curves. Each contact is independently
checked against the original model-space boundary image. Both face restrictions
participate in interval classification, through the existing face containment
rules for outer loops, holes, orientation, seams and singular trims.

Fractions map back through each curve's own parameter interval. Spatial and UV
curves can have different charts. Every emitted spatial span and its two
trimmed witnesses receive new continuous certificates against both original
surfaces, covering rounded knot insertion as well as parameter mapping.
Certified payloads are retained without spatial re-clipping or endpoint joining.
Exact duplicate payloads and covered points are removed; analytic face results
retain the shared legacy assembly policy.

Only exact natural boundary images skip UV clipping. Their controls must stay
on the corresponding natural sides, weights must have one sign, and trim
uncertainty must be zero. Other faces retain their actual loops. Normalized UV
contact tolerance is `1e-12` absolute and `1e-14` relative; proposed contacts that
miss the original boundary by more than the effective model tolerance fail.
Contact work is bounded by one million trim queries and 500,000 breakpoints
per proposal. Unsupported preparation and inconclusive certificates fail
explicitly through the established command error policy.

A fitted intersection touching a rectangular corner initially created a span
roughly `7e-9` model units long. The adapter preserves it as a point only when
all same-sign rational controls lie within a quarter model tolerance of a
certified original-curve point near a trim corner. The convex control hull bounds
the complete discarded span. Ordinary small curves away from corners retain
their curve representation.

[Retained qualification](smlib-surface-trim-intersections-comparison.json)
includes 12 actual C ABI cases, with 48 UV boundary contacts per normal and
complete AddressSanitizer job. Both source witnesses use independent surface
charts in three spatial planes. Native snapshots remain usable after their
event owner is freed; original source buffers remain unchanged. Independent
solutions of `x² + y³ + xy = 1` check the boundary positions and spatial images.
The kernel, SM_API and bridge are instrumented. Leak detection is disabled
because the kernel retains a process-global context.

Rust regressions cover partial rectangles, rectangular and rational circular
holes, both face restrictions, corner-only contacts, boundary overlaps,
independent XYZ/UV parameter charts, source chart scales `2^-360` and `2^360`,
and common negative weight gauges. Callback tests reject false boundary contacts
and propagate producer errors. Commands check operand order, names and colors,
selection, Undo/Redo, and atomic rejection of unsupported degree. Qualification
passes 272 native adapter tests, 92 native intersection command tests, 24 cutting
Split tests, six acceptance-hook tests and native/portable lint with warnings
denied.

The comparison retains complete native/portable responses for all 121 saved
intersection/cutting command records at absolute `1e-8`, relative `1e-12`, with
SDK comparisons kept separately. General trimmed-face regressions are independent
analytic tests, not fresh Rhino observations. Continuous source correspondence
does not prove complete contact discovery, face-region topology or injectivity.
Arbitrary conditioning, tangencies of fitted UV traces and speed relative to
Rhino remain unqualified.

```sh
cargo test -p viboceros-smlib --features native --test surface_intersections
cargo test -p viboceros-geometry --lib surface_surface_backend
cargo test -p viboceros-command --features native-smlib --lib curve_intersections::tests
cmake -S tools/usd_brep -B /tmp/surface-trims -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DVIBOCEROS_CURVE_INTERSECTION_DISTANCE_PATCH=ON \
  -DVIBOCEROS_CURVE_SELF_INTERSECTION_PAIRS_PATCH=ON
cmake --build /tmp/surface-trims --target viboceros_usd_brep_surface_trim_adapter_probe
/tmp/surface-trims/viboceros_usd_brep_surface_trim_adapter_probe
```

Use `RelWithDebInfo`, compiler flags `-fsanitize=address -fno-omit-frame-pointer`
and linker flags `-fsanitize=address` for ASan. See
`python3 -m tools.usd_brep.qualify_surface_trim_intersections --help` for the
comparison inputs.
