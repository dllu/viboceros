# Native curve-offset fitting and editing

Linux `native-smlib` builds use usd-brep's `SmOffsetCurve` evaluator and generic
`SmCurve::ApproximateCurve` for smooth planar NURBS offsets. The approximator's
parameter-matching mode checks corresponding parameters; analytic substitution
is disabled. `SmOffsetCurve` uses tangent × normal, so the bridge reverses the
shared normal × tangent signed distance.

The adapter translates and scales geometry, normalizes the parameter domain,
and canonicalizes common signed weight gauges before import. It restores the
original domain and model coordinates afterward. Requested closed outputs have
an exact endpoint seam; pieces cut at a closed source's kink stay open. Fillet
contact preparation reuses the same canonical working-curve helper.

Degree above 17, mixed-sign or excessive weight ratios, full-order interior
breaks, unrepresentable normalized charts and normalized offset distances below
`1e-12` retain the portable fitter. Native producer errors propagate. Analytic
line/circle/arc offsets and ellipse approximation retain their existing paths.

`CurveOffsetOperations` supplies optional smooth fitting plus trim, single-split,
multi-split and Bézier functions. Native editing covers source pieces, concave
corner trims, cyclic seam pieces, single concave-loop trimming, sharp extension
leaves and through-point candidates. OffsetMultiple region admission,
containment and nesting also reuse native Bézier decomposition and subdivision.
Existing geometry methods retain portable defaults; `offset_curve_parts`,
`offset_curve_through_point` and the native command/oracle paths select these
operations.

Shared code continues to decide source/region admission, poles, plane normal,
signed side, stationary points, offset cusps, source kinks, intersection/trim
selection, endpoint alignment, joins, attributes, selection and atomic history.
The native producer receives one eighth of the document absolute tolerance.
Shared code independently checks endpoints and 15 interior stations in every
interval of the combined source/output knot partitions against the source's
normal displacement. Output domains, closure, 8,192 spans and 24,577 controls
are checked. Native evaluation has a 1,048,576-call work budget to bound extreme
subdivision requests; output is published only after successful validation.

[Retained qualification](smlib-curve-offset-fit-comparison.json) includes:

- Six direct generic-approximator parabola cases, two parameter charts and
  tolerances `1e-5`, `1e-8`, `1e-10`, checked at 4,097 analytic stations.
- 48 actual C ABI parabola/rational-circle cases per normal and AddressSanitizer
  build, two charts, three planes, both signed sides and two tolerances. Exported
  nets receive independent de Boor checks at 1,025 stations. Native evaluations,
  source preservation, invalid values and extreme-tolerance rejection are checked.
  ASan covers SMLib, SM_API and the bridge; process-global context leak accounting
  alone is disabled.
- Four saved Rhino corner records at their existing `1e-7` comparison epsilon.
  These are replays of earlier SDK observations, not fresh captures.

Rust regressions check 72 analytic parabola combinations with extreme common
weight gauges, plus 18 closed rational-circle offsets across three planes and
three model scales. They check corresponding parameters, exact seams, signed
sides, cusp rejection, source retention and fallback behavior. Corner regressions
compare loci and closure for all four styles, including through-point selection.
The original 16 complete-net editing regressions still exercise native edits
with portable fitting. Shared callback regressions reject incorrect geometry
and domains, preserve producer errors and enforce source admission first.

Region regressions independently classify `C(t)=(9t(1-t),9t²(1-t))`, confirm
native subdivision, retain extreme common gauges and reject boundary ambiguity
and duplicated loops. The [earlier editing-only replay](smlib-curve-offset-comparison.json)
remains a historical comparison with the portable producer.

The [older sampling convenience probe](../tools/usd_brep/curve_offset_probe.cpp)
and [its output](smlib-curve-offset-probe.json) remain relevant: its circle branch
ignores normal orientation, and inherited-parameter parabola sampling returns
pointwise error `5.45e-5` at requested tolerance `1e-5`. The general
`N_CrvOffset`/`N_CrvOffsetApprox` declarations remain excluded by `NLIB_UNUSED`.
This integration uses the available generic curve approximator; it neither
substitutes the convenience sampler nor enables those unused blocks. See the
[usd-brep limitations](usd-brep-limitations.md).

Adaptive acceptance remains sampled, including upstream's reported gap; it is
not a continuous error certificate. Arbitrary conditioning, self-intersecting
or collapsing offsets and performance relative to Rhino remain unqualified.
Fresh captures remain pending the Wine licensing startup described in
[isocurve qualification](smlib-isocurves.md).

Configure the probe builds from `tools/usd_brep`. The normal build uses
`CMAKE_BUILD_TYPE=Release`. The sanitizer build uses `RelWithDebInfo`,
`CMAKE_CXX_FLAGS=-fsanitize=address -fno-omit-frame-pointer` and
`-fsanitize=address` for both executable and shared-library linker flags.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test curve_offset --test curve_offset_fit
cargo test --release -p viboceros-geometry curve_offset
cmake --build /tmp/viboceros-usd-brep-build --target \
  viboceros_usd_brep_offset_hermite_probe viboceros_usd_brep_offset_adapter_probe
cmake --build /tmp/viboceros-usd-brep-blend-asan --target \
  viboceros_usd_brep_offset_adapter_probe
python3 -m tools.usd_brep.qualify_curve_offset_fit \
  docs/smlib-curve-offset-fit-comparison.json \
  --probe /tmp/viboceros-usd-brep-build/viboceros_usd_brep_offset_adapter_probe \
  --asan /tmp/viboceros-usd-brep-blend-asan/viboceros_usd_brep_offset_adapter_probe \
  --hermite /tmp/viboceros-usd-brep-build/viboceros_usd_brep_offset_hermite_probe
```
