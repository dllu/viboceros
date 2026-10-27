# Adaptive Refit proposals

[Loft Refit](loft-refit.md) · [Current validation](loft-refit-stage-reserves-validation.json)

The adaptive APIs return certified geometry proposals. Rhino-compatible general
adaptive command preparation is still unfinished: fitting decisions can produce
different control nets even when the complete geometric error is bounded.

## Construction and joining

`try_propose_joined_refit` retains the original source and parameter clock. One
nine-station incremental length lookup supplies all dyadic split locations and
seven interior acceptance stations per interval. Finer stations interpolate that
original lookup. Endpoint tangents construct each cubic; a common positive handle
correction at the original lookup midpoint is tried before subdivision.
Correction stays within smooth parameter spans; an internal or ending C1 break
retains subdivision and its original limiting tangent. Half-tolerance samples
select proposals. An interval crossing an interior source C1 break also needs
a complete half-tolerance certificate, including features between stations.
This retains a reserve for C1 reconstruction when the corner's peak is missed
by the lookup stations. Smooth intervals and exact ending breaks retain their
full-tolerance interval bound. Every assembled result still needs its fresh
complete original-source certificate.

Joining uses an absolute curvature-vector difference of `0.01` in inverse model
units and the explicit corner angle. The general G2 query's additional relative
curvature allowance does not apply to this fitting rule. Adjacent handle lengths
set the retimed piece widths. A single curvature region is rebuilt by cubic
Hermite interpolation on fresh dyadic intervals in its retimed parameter clock.
Seven half-tolerance stations propose each interval; a continuous pointwise bound
within half tolerance also covers features between stations. The assembled curve
still requires a fresh complete bound against the original source. Multiple regions retain the
retimed handles and remove redundant C1 controls; their C0 region boundaries
remain authoritative during subsequent length-based chart matching.

The fitted-specific region policy is separate from ordinary and disabled-fitting
Loft matching. Raw assembly, joining and final certification are distinct: only
a fresh complete bound against the original source admits the final proposal.
Errors from these stages never receive separate additive allowances.

## Complete certificates

Affine, coordinate-projection, piecewise affine and cubic parameter proposals
all use the same continuous Bernstein image certificate. Cubic maps reuse cached
closest stations, with known fitting boundaries and one-sided parameter speeds
when available. Exact rational derivative bounds must prove strict parameter
order. Samples and anchor pairs guide proposals; they cannot establish acceptance
or hide an unsampled feature. Each certificate stage retains its work budget.
A [certified linear surrogate](curve-image-correspondence.md) can reduce the
cost of high-degree proofs while retaining the original source and fit.

Limits remain 128 pieces, depth 24, 512 source controls, source degree 16,
256 correspondence intervals, 255 closest queries and six refinement rounds.
Invalid inputs, exhausted budgets and unresolved proofs produce no partial fit or
source mutation. Nearly cancelling handle influences decline correction and let
subdivision continue. Neither image bounds nor parameter order certify topology.

## Remaining differences

The earlier 48 curved contexts, twelve source-clock transitions and 31
scale/curvature records remain comparison gates. Independent smooth, rational,
spatial and quintic sources now receive complete final proofs, but some resulting
control nets still differ from Rhino. Sixteen curvature-amplitude sweeps preserve
both joining modes and subdivision counterexamples. Sixty-four additional native
records cover styles, SDK/document commands, frames, reversal, weight gauges,
affine domains and model units. Absolute curvature rules can change region counts
when model units change; reversal also exposes a remaining spatial discrepancy.
The rebuild retains all 48 curved, twelve clock and 31 scale/curvature matches.
Numerical NURBS batch retention and complete raw-stage reserves recover all 24
original unequal-spacing cases at `1e-9`. Independent spacing and tolerance
holdouts are also recorded in the current validation.
All ten single-region knot sequences match Native; their rebuilt curves
also receive complete pointwise bounds against the saved Native curves within
5% of requested tolerance. Control differences up to `8.4e-8` remain, so those
bounds do not establish strict surface-record agreement. See the current
validation for the remaining counterexamples and source bindings.

`try_propose_adaptive_linear_refit` and `try_propose_joined_linear_refit` retain
an exact polygon proxy and interval-local length clock for straight-region source
images. Their unequal-region counterexamples remain tested. Historical
[raw-piece](loft-refit-adaptive-validation.json) and
[source-chart](loft-refit-original-source-validation.json) measurements retain
their original source bindings.

```sh
cargo test -p viboceros-geometry --lib loft::refit --frozen
cargo test -p viboceros-oracle --test loft_refit_adaptive_join_native \
  --test loft_refit_original_source_native --frozen
python3 -m unittest tools.rhino_oracle.test_loft_refit_join_modes \
  tools.rhino_oracle.test_loft_refit_adaptive_contexts
```
