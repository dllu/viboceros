# Native modeling curve import

`KernelCurve::from_nurbs` now accepts unclamped positive-weight NURBS data and
passes it directly to the existing usd-brep canonical curve constructor. The
constructor's IGES import routine clamps the supplied active interval. The bridge
checks that the native natural interval exactly retains the requested endpoints
and caches the actual exported controls, weights and knots. No curve fitting is
used, and no native pointer enters document data.

After native clamping, the constructor's knot fixup can merge or spread nearby
knots without adjusting controls. The bridge checks the exact clamped knot
sequence and uses existing `SetExpert` to restore that basis around the native
controls when necessary. A normalized native query chart handles tiny intervals
that the point evaluator rejects; original-domain snapshots remain exact. A
query chart is used only when distinct knots remain representable after mapping.
The snapshot readers now use existing full-knot getters instead of expanding
the tolerance-grouped unique-knot query. Surface pointer arrays are copied under
the kernel lock into owned storage.

Modeling import shares this operation across planar surfaces, straight/path/apex
extrusion and revolution. It retains the existing uniform negative-gauge sign
conversion, while the duplicate Rust active-interval extraction has been removed
from the adapter. The public curve-clamping helper added for that integration is
also removed; the geometry crate's private extractor remains in use by portable
curve edits. Surface clamping is unaffected.

Independent native regressions compare native evaluation and decoded snapshots
with the original curves at 129 parameters. Inputs include polynomial and
rational unclamped ends, a partially clamped cubic, repeated active interior
knots, periodic closed curves and uniform negative gauges. They check unchanged
source geometry, exact active domains and complete clamped output ends.
They also cover parameter scales `2^-360`, `1` and `2^360`, and three targeted
queries on a short parameter span with a ten-unit control bulge. The bulge case
guards against losing geometry while coalescing its two nearby knot groups.
[Sheet construction and direct input copies](smlib-modeling-basis.md) now share
the preservation adapter. Arbitrary downstream small-chart modeling remains
unqualified.

The pinned NLib fast evaluator limits canonical curves to degree 17 despite the
constructor header's stated upper bound of 32. The adapter now uses the existing
`NL_MAXDEG` build constant instead of advertising a degree-64 input range.
Boundary regressions cover degree 17 and rejection of 18, 32 and 33.

The default canonical importer requires all explicitly supplied weights in
`[0.001, 100]`; equal polynomial weights outside that range also fail before the
kernel removes their irrelevant gauge. The adapter uses the existing optional
unweighted constructor form for equal-weight polynomial curves. Positive and
uniformly negative gauges from `1e-308` to `1e308` are covered by the boundary
regression without rescaling geometry or changing global kernel limits.
For unequal weights, checks occur before native clamping, so inactive source
controls can also trigger rejection. The adapter does not implement another
clamping algorithm to bypass this restriction.
General gauge conditioning and inactive invalid-weight handling remain open.
Direct negative weights and discontinuous active interior knots are rejected;
the latter guard prevents native knot sanitation from averaging discontinuous
controls. See the [separate limitations ledger](usd-brep-limitations.md).

The [retained replay](smlib-curve-import-comparison.json) and
[native references](smlib-curve-import-native-reference.json) cover 23 existing
Rhino observations across extrusion, apex, revolution, curve-path extrusion and
planar surfaces. All execute successfully; 22 complete raw records match at
`1e-10` absolute / `1e-12` relative epsilon. The existing unclamped-path raw
representation difference remains, while all seven curve-path sample grids
match. No new Rhino capture or geometry normalization was used for this replay.

Release validation passed 1,310 command tests and 64 native bridge tests (three
tests ignored across those suites). Native clippy with warnings denied and
workspace formatting checks passed. The adapter restrictions and ingress scope
above remain part of the qualification.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test curve_import
```
