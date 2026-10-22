# Divide and arc-length follow-up checkpoints

[Historical checkpoint](../../validation-checkpoint.md)

## Divide checkpoint

[Divide](../../divide-command.md) adds equal-chord stations, Split, DeleteRemainder and
GroupOutput with atomic output staging, one-step Undo and complete argument
retention through curve picking. Native captures establish current-layer point
attributes, per-source point grouping, ungrouped split outputs, circular-NURBS
arc simplification and the Length MarkEnds remainder policy.

The final workspace release run passes 5,631 Rust tests with zero failures and
38 ignored tests. All 1,077 Python tests, Clippy with warnings denied and
formatting pass. The permanent replay checks every output field in 30 native
recipes: 27 match at 1e-9 absolute plus 1e-12 relative epsilon; three circular
length cases require 5e-7 absolute epsilon, with maximum difference
1.1752123363351075e-7. No fields are excluded and no timing comparison was made.
[Provenance](../../divide-command-provenance.json) retains source and validation hashes.
Tighter circular length parity, broad rational-curve qualification, bare-value
prompting, preview, seam/direction changes, SubCrv and performance remain open.

## Divide input checkpoint

[Bare Divide input](../../divide-input.md) now gathers curves and opens a typed
numeric/options stage with cached viewport station markers. Count, length and
chord values remain available when switching modes. Edits and cancellation
create no geometry or history; Enter accepts one transaction. Noncurve
preselection is excluded and restored on cancellation. Source geometry identity,
selection order and tolerance qualify the preview; stale markers disappear and
stale acceptance is rejected.

Five application regressions and two prompt-parser tests cover these paths.
The complete workspace release run passes 5,638 Rust tests with zero failures
and 38 ignored tests. All 1,077 Python tests, Clippy with warnings denied,
formatting and diff checks pass. The existing 30 native Divide output recipes
still pass their documented per-case epsilons. No new native UI or timing
capture was made. [Provenance](../../divide-input-provenance.json) binds the final
sources and logs. Native preview styling, persistent defaults, seam/direction
changes and SubCrv remain open.

## Curved Divide checkpoint

[Curved Divide qualification](../../divide-curved.md) adds 63 licensed Rhino recipes
under private Xvfb, plus three SDK source-type observations. Exact rational
intervals replace the chord root-depth cutoff; denominator validation no longer
depends on arc-length integration. A bounded-degree Sturm check rejects false
near-contact roots. The rational line with weight ratio 2^100 now produces its
missing midpoint station, and non-dyadic turnaround chord division succeeds.

Source inspection corrected the oracle's earlier Circle-to-NURBS promotion.
Explicit NURBS-circle Split captures then exposed an incorrect whole-circle
analytic carrier; trimming now retains original rational domains and samples.
All 55 regular curved recipes and four NURBS-circle recipes match within their
documented per-case epsilons. Two extreme point cases match; both extreme Split
cases remain full-field diagnostics with unresolved native output counts or
parameterization. They are not included in matching point qualification.

The final complete release run passes 5,645 Rust tests with zero failures and
38 ignored tests. All 1,077 Python tests, Clippy with warnings denied,
formatting and diff checks pass. [Provenance](../../divide-curved-provenance.json)
records sources, captured helper versions, raw diagnostics and final reports.
Cusp arc-length integration, native extreme Split policies, general extreme
domains and broad geometry/performance qualification remain open.

## Cusp arc-length checkpoint

[Global integration refinement](../../arc-length-cusps.md) fixes the non-dyadic
turnaround length failure and a signed-cancellation error outside the requested
absolute tolerance. Exact finite sums track interval values and estimated
errors; the largest refinable error drives subdivision. Stable intervals at
roundoff remain in the global sum while other intervals continue refining.

Five new kernel regressions verify a closed-form cusp integral, signed
cancellation, bounded roundoff failure, refinement beside a large constant
interval, and exact backtracking NURBS length/stations across four parameter
domains. Eighteen fresh licensed Rhino Divide recipes under private Xvfb now
execute locally, compared with zero successes previously. Fifteen match at
5e-7 absolute plus 1e-12 relative epsilon. Three cusp inversion records remain
full-field diagnostics; no output is fitted to hide those differences.

The final release run passes 5,651 Rust tests with zero failures and 38 ignored
tests. All 1,077 Python tests, Clippy with warnings denied, formatting and diff
checks pass. [Provenance](../../arc-length-cusp-provenance.json) binds current sources,
native recipes, rejected results and final comparison. Unsampled sharp features,
extreme rational weights, native cusp inversion and full geometry/performance
qualification remain open.

## Rational line arc-length checkpoint

[Degree-one rational arc length](../../rational-line-arc-length.md) uses straight
span geometry and exact projective distance/parameter expressions. Extreme
weights no longer hide almost all motion from derivative quadrature. The
sampler carries span metadata through polycurve parameter maps, retains direct
degree-one domains, and bypasses numerical integration and lookup tables.

Four new kernel regressions cover extreme/reversed/negative weights, corners,
poles, trimmed domains, forward/inverse queries and polycurve sampling. An
updated regression verifies analytic measurement/sampling of a subnormal span
that integration normalization still rejects. A command regression checks
accurate point placement and atomic rejection of an unrepresentable polycurve
Split cut. Divide now validates cut coordinates before trimming.

Thirty-two fresh licensed Rhino recipes under private Xvfb retain sixteen
moderate-weight matches at 5e-7 absolute plus 1e-12 relative epsilon. Sixteen
extreme-weight records remain full-field diagnostics: native macros end with
zero output, local point geometry remains accurate, and six local Split cases
reject unrepresentable cuts. Native-only command history and completion-state
inspection confirms the zero outputs are not unfinished getters.

The final release run passes 5,657 Rust tests with zero failures and 38 ignored
tests. All 1,077 Python tests, Clippy with warnings denied, formatting, diff
checks and source-pin validation pass. [Provenance](../../rational-line-provenance.json)
records exact sources, captured helper versions and rejected intermediate
results. Extreme native compatibility, parameter representability, general
rational integration and full geometry/performance qualification remain open.
