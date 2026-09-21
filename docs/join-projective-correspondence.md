# Projective parameter correspondence for surface Join

[Kernel](brep-edge-joining.md) · [Command](commands/join.md) ·
[Complete replay report](join-projective-correspondence-comparison.json)

This page records the original projective implementation audit. The later
[short-overlap follow-up](join-short-overlaps.md) resolves its four orientation
failures, bringing this fixture to 44 full matches; its other 56 rows are unchanged.
The original report below remains historical evidence.

Curved boundaries can now join when a positive projective parameter change
relates their rational representations. This extends the previous
[affine span certificate](join-curved-certificates.md) without moving control
points, refitting surfaces, or sampling a curve into an accepted match.

## Correspondence and proof

On normalized domains, `F(t) = c*t/(1-t+c*t)` is an increasing bijection of
`[0,1]` when `c > 0`. Its endpoint derivatives are `c` and `1/c`. Exact endpoint
derivatives of the two curves therefore suggest up to two factors. At stationary
ends, equal-degree nets can instead suggest a factor from adjacent weights.
These are proposals only: nonparallel tangents, rounded controls, or a misleading
weight ratio cannot bypass the subsequent whole-curve test.

The matcher first tries affine correspondence. Otherwise it partitions at the
union of A's knots and the exact inverse images of B's knots. On each interval
`[l,r]`, B is extracted over `[F(l),F(r)]`. Its local projective factor is
`k = (1-r+c*r)/(1-l+c*l)`. Multiplying homogeneous control `i` by `k^i` composes
that span with the parameter map. Coordinates and weights are both multiplied;
the common projective denominator cancels. The resulting positive rational
difference has the same exact Bernstein hull/subdivision certificate as before.

Mapped knots and factors remain arbitrary-precision rationals, even when no
binary64 value represents them. The accepted bound covers complete oriented
curve loci, not necessarily equal native parameters. Independent equal-parameter
tests now explicitly request the identity map rather than assuming every locus
bound has that stronger meaning.

The existing degree-16, depth-16 and shared Join work limits remain in force.
Each proposal, extraction, composition, product and subdivision is charged;
input geometry stays immutable on exhaustion. Existing fast common-basis and
straight-locus certificates are unchanged. Natural-boundary uncertainty may use
the tighter proven projective correspondence. This is not arbitrary
reparameterization or a minimum-distance/Hausdorff solver.

Tests exercise independent degrees and knot refinements, reversal, parameter
speeds from `1e-100` to `1e100`, negative common gauges, stationary endpoints,
exact inverse composition, strict normal-gap rejection and budget exhaustion.
An independent exact Cox–de Boor evaluator checks the composed correspondence.
A polynomial `B(s)=A(s^2)` test deliberately remains unmatched: the positive
projective family does not include its zero-speed endpoint correspondence.

## Actual Rhino commands

The [60 requests](../tools/rhino_oracle/fixtures/join_projective_correspondence.json)
and [raw observations](../tools/rhino_oracle/observations/join_projective_correspondence.json)
were measured using owned per-source 3DM exports in a private-Xvfb session with
Rhino 8.32.26160.13001. Both commands and selection modes are covered. The previous
release matched four records; the new implementation fully matches forty at
`1e-10` absolute / `1e-12` relative epsilon. Every discovery result is retained.

The remaining twenty records are not normalized into matches:

- Eight differ only in face areas. Their planar parabola extrusion has exact
  area `45*(sqrt(2)+asinh(1))`, approximately
  `103.30142172266871333154341221352706744`. An independent 80-digit evaluation
  places native within `7.5e-15`; Rhino differs by about `1.14e-10` or `4.95e-10`.
  JoinCopy originals show the same integration difference. All other fields match.
- Four join the same geometry but Rhino records edge uncertainty
  `0.00024793388429752067`, versus native's certified `0.00025`. At native B
  parameter `1/3`, the exact normal separation is `0.00025`, and every point of
  the retained curve lies in `z=0`. No parameter correspondence can make that
  normal separation smaller. The native bound is not reduced to the estimate.
- Four translated-gap cases expose a short-overlap topology conflict. The long
  curved mate requires opposite face senses, while tiny overlapping straight
  side boundaries require the other orientation. Native proposes both kinds of
  mate and rejects the inconsistent cycle atomically. Rhino keeps just the long
  mate, averages the boundary to `z=0.00025`, and returns seven edges. The older
  native implementation instead joined tiny side pieces into the wrong ten-edge
  result. Short-overlap discovery/rebuilding still needs an evidence-supported
  ownership rule; the failure is preserved as a classified error, not a value.
- Four use `B(s)=A(s^2)`: the geometry is identical but the correspondence is not
  projective. Rhino joins; native leaves separate sheets. The quadratic-composed
  surface also has an independent Rhino area discrepancy in JoinCopy originals.

Across twelve surface-command fixtures, **551 of 702** records fully match,
36 retain native execution failures and 115 have other differences. The earlier
642 rows keep exactly their previous flags, residuals and errors. The report
records source, fixture, observation, executable and shared-artifact hashes, plus
the baseline result. Command-harness timings do not establish kernel speed parity.

```sh
cargo test --release -p viboceros-geometry join_edges
cargo test --release -p viboceros-oracle projective_discovery
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/join_projective_correspondence.json --observations tools/rhino_oracle/observations/join_projective_correspondence.json --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

Replay intentionally exits 1 for the preserved differences and failures.
Ordered corner clustering, short overlaps and more general curved correspondence
remain open implementation work.

Verification passed 2,836 Rust tests (20 ignored), 232 Python tests, formatting,
and Clippy/Rustdoc with warnings denied. README remains concise and unchanged.
