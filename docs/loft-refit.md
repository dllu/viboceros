# Loft Refit

[Loft](loft.md) · [Section rebuilding](loft-rebuild.md)

`Loft Refit=0.001` refits straight and polygon profiles to polynomial cubics
before lofting. Open, closed, rational and unclamped profiles are supported,
including spatial polygons and different corner counts. Sources, selection and
attributes remain intact; accepting creates one undo step. Interactive Loft
accepts `Refit` followed by a tolerance, or `RefitTolerance=<value>`.
`Simplify=Refit` uses the current tolerance, defaulting to `1e-9`;
`Simplify=None` disables simplification. Rebuild and Refit are mutually exclusive.
Zero leaves the original profiles unchanged.

Positive Refit requires same-sign weights and continuous, exactly straight
regions with ordered controls. Exact rational span extraction admits the source;
sampled linearity is insufficient. Constant spans and redundant collinear
vertices disappear; reversals remain. Internal bends must exceed the legacy
one-degree corner threshold, with a conservative floating boundary margin.
Shallower bends still require the adaptive smooth fitter.
Mixed open/closed profile sets and explicit JSON Refit angle overrides fail.

[Smooth proposals](loft-refit-candidates.md) are also supported for sets of
at least two clamped single-Bezier profiles. The endpoint-tangent cubic, optionally
corrected by a common handle scale, must
have a continuous positional bound within half the requested tolerance; the
tolerance must be smaller than each endpoint chord. For example, `Refit=0.1`
can fit an open polynomial cubic while tighter settings may need the pending
subdivision rules. Unsupported fits fail before document mutation.

Degree-one proxies propose a shared region chart before cubic construction.
The final controls use exact source vertices, including unclamped endpoints;
one final rounding per control bounds the complete image distance by the requested
tolerance. Direct construction in the shared basis avoids knot-insertion drift.
This preserves geometric images, **not their original parameter correspondence**.
Segment extraction supports degrees up to 16 under the existing work/bit budgets;
whole straight profiles retain their previous path. Source and shared sections
are limited to 512 controls and 256 profiles. Unrepresentable tolerances or region
lengths fail explicitly.

[Polygon validation](loft-refit-polylines-validation.json) covers the new command
behavior and full native surface records. Adaptive smooth Refit still needs its
subdivision
and piece-joining policy; the separate [source-parameter approximation](cubic-approximation.md)
is not a substitute. [Profile-set preparation](loft-refit-preparation.md)
retains the paired-input reference and its limits.

Earlier [straight-profile checks](loft-refit-lines-validation.json) and the
[investigation](loft-refit-validation.json) retain historical source bindings.
Native data and content-addressed producer archives remain immutable.

```sh
cargo test -p viboceros-oracle --test loft_refit_polylines_native --test loft_refit_lines_native --test loft_refit_native
cargo test -p viboceros-oracle --features native-smlib --test loft_refit_polylines_native --test loft_refit_lines_native --test loft_refit_native
python3 -m unittest tools.rhino_oracle.test_loft_refit_polylines
```
