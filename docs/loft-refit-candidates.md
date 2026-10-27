# Smooth Loft Refit proposals

[Loft Refit](loft-refit.md) · [Validation](loft-refit-candidates-validation.json)

Refit admits an endpoint-tangent cubic for sets of at least two
clamped profiles without resolved tangent breaks. For chord length `d` and the cosine `c` between
the chord and an endpoint tangent, the initial handle length is
`d * (2 - c²) / 3`. The shared constructor preserves endpoint positions and
limiting tangent directions before rounding. Its output uses polynomial cubics.
When the initial proposal fails, a
[common handle correction](loft-refit-adjustment.md) can improve the fit.
That correction remains qualified for single-Bezier sources. Multispan profiles
use the initial proposal; if it fails, their adaptive fitting remains unfinished.

Acceptance requires a continuous Bernstein bound on the rounded candidate,
compared with its complete source. An affine parameter correspondence is tried
first, followed by a [nonlinear projected map](curve-image-correspondence.md).
Closest-point samples can still propose a strictly increasing map if needed.
Every interval of that map must pass the complete bound. Single-Bezier fits
retain the half-tolerance reserve; eight native boundary pairs confirm it for
two symmetric profiles under scale, frame and domain changes.

Multispan initial proposals can instead pass quarter, midpoint and three-quarter
length stations within half tolerance, followed by a complete bound within the
full requested tolerance. Stations use the existing nine-point incremental batch
at fractional stopping precision `1e-4`. An unsampled feature exceeding tolerance
still rejects the fit. This independent selection rule matches 82 complete native
records across source clocks, affine domains, reversal, degree elevation,
negative weight gauges, frames, all five styles and document commands at `1e-9`.
[Initial-station validation](loft-refit-initial-stations-validation.json) also
retains 52 public deviation measurements: their maxima do not change with source
clocks, so that metric alone cannot explain adaptive subdivision differences.
The nonlinear proof now certifies all 18 native initial fits among 24 curved
transition holdouts; six still need subdivision. Native fitting clocks agree
with the existing incremental batch, so no sampling or tolerance rule changed.
These observations do not establish the general native adaptive fitting policy.

The tolerance must be smaller than every endpoint chord. The existing 256-profile
limit applies; certificate degree, arithmetic and work limits remain unchanged.
The piecewise affine fallback retains natural source knots and bisects the largest remaining
parameter gaps over at most six attempts. Natural boundaries share the existing
256-interval budget, with at most 255 cached closest-point queries per profile;
each attempt uses the bounded certificate.
If all knots cannot fit in that budget or fractional chart, the former uniform
proposal remains available.
Coincident endpoints, closed profiles, unresolved certificates,
and fits needing subdivision retain explicit errors. Straight and
polygon fitting keep their established paths. A single smooth input uses a
different native fitting branch and remains unsupported.

[Adaptive piece proposals](loft-refit-adaptive.md) have a separate certified
kernel API, including construction from original source charts. General
adaptive preparation is not yet qualified for the Loft command.

[Multispan validation](loft-refit-multispan-validation.json) covers shallow bends,
rational region clocks, curvature changes, domain/frame transformations and
commands. Curvature changes alone do not require separate fits. Resolved tangent
corners and positional jumps retain their existing policies, even when a nearby
cubic has a complete positional bound.

Native captures cover 135 cases, including spatial cubics, rational profiles,
tolerance sweeps, mixed companions, negative weight gauges, all five styles,
closed lofts and point ends. Every admitted case compares complete surface
records at `1e-9` in that initial corpus; the linked record retains the initial-proposal
milestone. [Adjustment validation](loft-refit-adjustment-validation.json) records
the expanded implementation, elliptical `1e-8` reserve and remaining gaps. The command
path preserves sources and selection, creates one undo step, and leaves history
intact when a tighter fit fails. General smooth fitting remains unfinished.

```sh
cargo test -p viboceros-oracle --test loft_refit_coarse_native
cargo test -p viboceros-geometry --lib curve_fit::g1
python3 -m unittest tools.rhino_oracle.test_loft_refit_candidates
```
