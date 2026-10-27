# Loft Refit

[Loft](loft.md) · [Section rebuilding](loft-rebuild.md)

`Loft Refit=0.001` refits profiles to polynomial cubics before lofting.
Straight and polygon profiles support open, closed, rational, unclamped and
spatial sources, including different corner counts. Open smooth profiles can
use certified adaptive subdivision and joining. Sources, selection and
attributes remain intact; accepting creates one undo step.

Interactive Loft accepts `Refit` followed by a tolerance, or
`RefitTolerance=<value>`. `Simplify=Refit` uses the current tolerance, defaulting
to `1e-9`; `Simplify=None` disables simplification. Rebuild and Refit are mutually
exclusive. Tolerances through `2 * sqrt(f64::EPSILON)` (approximately `2.98e-8`),
including zero and equality, retain the original definitions while preparing
their common basis and region correspondence. Choose a larger value to fit.

## Fitting and guarantees

Straight regions require same-sign weights and exact rational collinearity and
control order. Constant spans and redundant collinear vertices disappear;
reversals remain. Region matching precedes direct shared-basis construction,
with one final rounding bound against the exact source vertices.

Smooth profiles first try a certified [initial cubic](loft-refit-candidates.md)
and its qualified common-handle correction. Tighter fits use
[adaptive subdivision, joining and reconstruction](loft-refit-adaptive.md).
Every final curve requires a complete positional bound against its original
source; samples only guide proposals. Adaptive common-basis edits require a
fresh source certificate unless the curve is unchanged. Supplied native backend
operations participate in matching and propagate errors before document mutation.
Parameter correspondence can change. These bounds do not certify topology.

Generic adaptive fitting requires at least two open regular profiles, degree at
most 16, tolerance below every endpoint chord, and no resolved tangent breaks.
Closed smooth sources, mixed open/closed sets, exhausted budgets and unresolved
proofs remain unsupported. Explicit [API angles](loft-refit-angles.md) control
corner retention. Sources and shared sections have a 512-control limit; Loft
accepts at most 256 profiles.

## Reference comparisons

[Adaptive command validation](adaptive-loft-preparation-validation.json) compares
the actual SDK and document paths with
saved Rhino records. Strict control-net comparisons retain their existing
thresholds. New admissions also require complete polynomial surface bounds
within the requested tolerance, including rounding in verification-only knot
refinement. Different control nets can therefore pass a geometric comparison
while remaining explicit strict mismatches.

[Polygon validation](loft-refit-polylines-validation.json) and the earlier
[preparation investigation](loft-refit-preparation.md) retain their historical
source bindings. Native captures and producer archives remain immutable.

```sh
cargo test -p viboceros-oracle --test loft_refit_adaptive_preparation_native --frozen
cargo test -p viboceros-command --lib loft::refit_tests --frozen
python3 -m unittest tools.rhino_oracle.test_loft_refit_adaptive_admission
```
