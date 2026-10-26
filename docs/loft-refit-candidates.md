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
first; closest-point samples can propose a strictly increasing map if needed.
Every interval of that map must also pass the complete bound, within half the
requested tolerance. Eight native boundary pairs
confirm this reserve for two symmetric profiles under scale, frame and domain
changes. This sufficient certificate can reject fits that Rhino accepts using
other correspondences; it never substitutes sampled agreement for a bound.

The tolerance must be smaller than every endpoint chord. The existing 256-profile
limit applies; certificate degree, arithmetic and work limits remain unchanged.
Map refinement allows 8–256 intervals over six attempts, caching at most 255
closest-point queries per profile; each attempt uses the bounded certificate.
Coincident endpoints, closed profiles, unresolved certificates,
and fits needing subdivision retain explicit errors. Straight and
polygon fitting keep their established paths. A single smooth input uses a
different native fitting branch and remains unsupported.

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
