# Native section matching for Loft, Sweep1 and Pipe

Linux `native-smlib` builds prepare common section bases using the existing
usd-brep clamping, exact degree elevation and knot-refinement adapters. Loft,
Sweep1 and Pipe's sweep-model paths now use this preparation. Both ordered API
and command Loft paths share it, including 3DM construction probes.

The geometry layer shares count/basis planning, weight scaling, normalized
section domains and maximum knot multiplicities through caller-supplied exact
operations. It validates returned degrees, active intervals, control budgets
and common full knot vectors. Sources are immutable; native handles never enter
document state. Command helpers propagate kernel failures through the existing
typed error conversion before document mutation.

Before exact basis edits, Loft now shares the
[region-based parameter preparation](loft.md#geometry-and-modules) between
backends. Full-multiplicity curvature breaks can be matched; simple smooth knots
retain their original correspondence. Both backends preserve the same remaining
symmetric near-tie limitation documented in [Loft validation](loft-validation.md).

Loft keeps its per-section weight scale and shared endpoint-weight normalization,
including the first-section knot policy that can move later unequal-end-weight
profiles. Sweep1 keeps its common weight scale and positive-weight requirement.
Section ordering, seam/orientation policy, rail frames, local/global blending,
automatic Loft end handles, periodic interpolation, adaptive sweep fitting,
crease topology and final domains retain their existing implementations.
Sweep fitting retains its existing solver. Loft now delegates its interpolation
to [native basis and matrix routines](smlib-loft-interpolation.md); portable builds
retain faer. No new fitting solver is added.

The native skin convenience producer has different open control counts and
closed end-condition policy, so replacing the complete Loft with it would not
preserve the established Rhino behavior. The
[limitations ledger](usd-brep-limitations.md) retains that source-audited
distinction. This integration moves the exact section edits to usd-brep;
general native skin construction remains unqualified.

The [native Loft tests](../crates/viboceros-smlib/tests/loft_basis.rs) compare all
72 permanent API, command-input and end-weight fixture definitions against the
portable ordered model, including all five styles, open/closed modes, mixed
degrees/knots and unequal endpoint weights. Each comparison checks definitions
and a 17×17 parameter grid. These comparisons include the three existing
near-equal-weight diagnostics as native-versus-portable checks, not new Rhino
parity claims. Additional tests check exact common layouts, source purity,
binary profile charts `2^-360`, `1`, `2^360` and common gauges `1e-300`, `1`,
`1e300`, `-1e300`.

The [native sweep test](../crates/viboceros-smlib/tests/section_sweep.rs) compares
mixed-degree, unclamped sections with both Local/Global blending and
Freeform/Roadlike frames. Sampled sections and the fitted surface are checked
against the portable model. Shared-policy tests verify operation order, budget
rejection before backend entry, layout validation and immutable inputs.

The original native section-basis qualification run passed 3,595 tests (11 ignored),
with two additional debug policy regressions passing: 3,597 distinct cases in
total. Workspace lint passes in both feature configurations with warnings denied,
and formatting passes. The [comparison](smlib-section-bases-comparison.json)
retains all 72 full Loft API/command records against a separately built portable
oracle at absolute `1e-9`, relative `1e-12`; every record matches. All three
retained closed-circle Sweep1 records also match that portable baseline.

The original 365 retained Rhino records still yield 364 complete matches and the
documented whole-domain surface outer-knot padding discrepancy. Of the additional
three closed-circle Sweep1 records, the refitted world-space case fully matches
Rhino. The other two retain their previously documented off-surface closest-point
residuals and dense nonrational versus exact rational circle-basis difference;
see [Sweep1 qualification](sweep1/validation.md). Their raw failures remain recorded. Native
section matching does not change those existing geometry/representation policies.

Native degree, resource, chart and weight restrictions remain those of the
existing exact-edit adapters, including the documented full-order degree-elevation
failure. These finite checks do not certify arbitrary profile/weight ratios,
continuous fit error or kernel speedups. Fresh Rhino 8.32 captures for the new
Loft region policy are retained with their frozen producer sources; see
[Loft validation](loft-validation.md).

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test -p viboceros-smlib \
  --features native --test loft_basis --test section_sweep
```
