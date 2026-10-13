# Native curve-offset editing

Linux `native-smlib` builds reuse usd-brep trim and split adapters during
Offset and OffsetMultiple NURBS corner construction. This covers source pieces,
concave corner trims, cyclic seam pieces, single concave-loop trimming, sharp
extension leaves, and through-point candidates. The offset geometry oracle
selects the same producer.

`CurveOffsetOperations` supplies trim, single-split and multi-split functions.
Geometry callers use `try_offset_with_operations`,
`try_offset_parts_with_operations` or `try_offset_through_point_with_operations`.
Existing methods keep portable defaults. The bridge exposes
`curve_offset_operations`, `offset_curve_parts` and
`offset_curve_through_point`; native failures retain their message. Editing
above the pinned degree-17 limit retains the existing portable producer.

Shared code still decides source/region admission, plane normal, signed side,
nesting, source kinks, intersection/trim selection, endpoint alignment, joins,
output budgets, attributes, selection and atomic history. Analytic line/circle/
arc offsets remain direct constructions. Smooth NURBS/ellipse images retain
the existing adaptive Hermite model and independent pointwise checks; this
change adds no new offset approximation, slicing or corner algorithm.

Native regressions compare complete segment bases, controls, weights and
independent samples for 16 open/closed, positive/negative-distance cases across
all four corner styles. Through-point results and invalid-input rejection
retain sources. [Saved-producer replay](smlib-curve-offset-comparison.json)
compares all four existing offset-corner records at absolute `1e-9`, relative
`1e-12`. These compare the prior portable producer, not fresh Rhino captures.
Existing command tests separately retain geometry, options, source attributes,
selection, nesting and history checks.

The [direct upstream probe](../tools/usd_brep/curve_offset_probe.cpp) and
[its output](smlib-curve-offset-probe.json) explain why the sampling convenience
producer is not a qualified replacement for the shared offset fitter:

- General adaptive/functional producers are declared but excluded by
  `NLIB_UNUSED`; only the point-sampling producer is exported in the default
  build.
- Its circle branch returns `r + d` for either requested normal orientation.
  A counterclockwise radius-two XY circle with normal `+Z` and positive
  distance `0.125` should have radius `1.875`, but returns `2.125`.
- A quadratic parabola offset with inherited parameters returns success at
  tolerance `1e-5` while an independent analytic/de Boor comparison gives
  corresponding-parameter error `5.45e-5`. This is a pointwise qualification
  limit, not a demonstrated violation of a geometric closest-distance bound.

The probe uses unit weights for polynomial output, whose stored weight slots
are not an explicit rational weight array. Existing native extraction imports
already preserve this distinction. The exact findings and relevant source
links are retained in the [usd-brep ledger](usd-brep-limitations.md).

The optimized geometry/command/native-bridge suite passes 3,663 tests
(11 ignored). Workspace lint passes with warnings denied in both feature
configurations; formatting passes. The standalone probe builds and records its
upstream diagnostics without changing the pinned kernel.

The existing 8,192-span fit/region limits and command output budgets remain.
Adaptive acceptance remains sampled; arbitrary conditioning, self-intersecting
or collapsing offset behavior and performance relative to Rhino remain
unqualified. Fresh captures remain pending the Wine licensing startup described
in [isocurve qualification](smlib-isocurves.md).

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test curve_offset
cmake --build /tmp/viboceros-usd-brep-build \
  --target viboceros_usd_brep_curve_offset_probe
```
