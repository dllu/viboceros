# Native UV drop accuracy

The native curve-to-surface dropper now keeps refining a failed UV span instead
of accepting it merely because its parameter interval is short. This repair
addresses inaccurate derived end-cap trims found while developing surface fillets.
It preserves the defining 3D curves and surfaces and the caller's distance budget.

There were two independent acceptance problems in the pinned SMLib producer:

- A failed Hermite accuracy test was overridden when the span was shorter than
  `1/8000` of the source interval. A short parameter interval does not bound a
  model-space error.
- Three uniform test points, and the separate isoparametric shortcut, could miss
  an entire narrow surface feature. The shortcut counted source knots to choose
  uniform stations without testing the actual knot parameters.

The repair removes the failed-test override. It retains the original three
Hermite stations and adds stations within each crossed surface knot span. The
isoparametric shortcut also tests actual source knots and crossed surface spans.
Crossings come from the existing native curve/line intersection machinery on an
owned 2D B-spline representation of the candidate Hermite curve. The Hermite
control hull excludes irrelevant knot lines. Crossings remain separate even when
close together; interval classification can merge such boundaries. Only exactly
duplicate parameters are removed. No surface geometry, source curve, normal-drop
criterion or requested tolerance is changed.

Subdivision uses an overflow-resistant midpoint and requires representable
progress both before and after the existing point-drop retry. When progress is
impossible, it follows the existing failed-drop cleanup path. The old six point
drop retries remain in place. This is sampled accuracy checking, not a continuous
certificate or a completeness proof for arbitrary surfaces.

The independent regression uses a regular parabolic cylinder with a localized
UV shear:

```text
S(u,v) = (u, v + a * beta3((u-c)/h), u^2)
C(t)   = (t, 0.5, t^2)
UV(t)  = (t, 0.5 - a * beta3((t-c)/h))
```

Here `beta3` is a centered cardinal cubic B-spline and `a = 0.001`. Both source
objects are exact polynomial NURBS; the surface has one unambiguous drop solution,
no seams and no poles. Lateral deviation from `y = 0.5` independently bounds the
distance to the entire source curve, so reparameterizing cannot explain away a
bad result. Dense stations on every output knot span and on the independent
feature chart detect both inaccurate local fits and false straight UV lines.

The direct probe covers 270 recipes: three feature centers, five widths from
`0.01` to `0.000001`, three physical scales, three axis placements and both curve
orientations. It checks unchanged native source definitions, finite bounded UV
points, segment count and the original `1e-8 * scale` spatial budget. It destroys
both native inputs before evaluating the independently owned UV outputs. The
preceding producer passes 18 recipes and fails 252, with observed errors up to
`0.000667 * scale`. The repaired producer passes all 270 normally and under AddressSanitizer
with leak detection disabled. A separate fillet probe also passes 56 geometry
and ownership cases and 10 rejected requests in both builds. Complete results
and shared caller checks are retained in the
[validation record](native-drop-curve-accuracy-validation.json).

A four-case fillet diagnostic also retains complete before/after surface, edge,
vertex and trim definitions. Defining surface/edge geometry, edge intervals,
vertices and trim connectivity/orientation remain exactly unchanged. In the
normalized extended case, the previously observed cap-to-edge witness gaps
`3.37e-8` and `1.20e-7` fall to `3.50e-9` and `2.48e-9`. The maximum sampled
synchronized discrepancies on these caps are `4.94e-9` and `7.01e-9`, against the
original `1e-8` caller budget. These are sampled diagnostics. Changes in native
tolerance metadata remain visible in the snapshots and do not set the acceptance
budget for this comparison.

The saved SDK audit retains five successful fillet recipes, with maximum
sampled coordinate difference `5.07e-9` at 20 closest points. The curved extended
recipe remains a failure: `adaptive numerical integration did not converge at
the requested tolerance`. It is not counted as a successful match. Complete raw
NURBS/topology discrepancies remain retained. The shared bridge and command
suites pass 430 and 1,600 tests respectively, alongside workspace lint and
formatting checks.

This repair does not finish [FilletSrf](native-surface-fillets.md). Curved support
extension, boundary policy and the document command workflow still need work.
General drop completeness and performance remain unqualified. The upstream USD,
Python-binding, OCCT, `repo.sh`/`prog_test` and GUI harnesses were not run; the local
ARM build executes the actual SMLib and SM_API libraries.

```sh
cmake --build <native-build> --target viboceros_smlib_drop_curve_probe -j10
<native-build>/viboceros_smlib_drop_curve_probe
cargo test --release -p viboceros-smlib --features native
cargo test --release -p viboceros-command --features native-smlib
```
